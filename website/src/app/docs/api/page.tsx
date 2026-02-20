"use client";

import { CodeBlock } from "@/components/ui/code-block";
import { Badge } from "@/components/ui/badge";

interface Endpoint {
  method: string;
  path: string;
  description: string;
  response?: string;
}

const restEndpoints: Endpoint[] = [
  {
    method: "GET",
    path: "/api/primes",
    description: "List discovered primes with pagination and filtering.",
    response: `{
  "primes": [
    {
      "id": 1,
      "form": "factorial",
      "expression": "147855! + 1",
      "digits": 636919,
      "proof_type": "pocklington",
      "certificate": { ... },
      "discovered_at": "2026-02-14T12:00:00Z"
    }
  ],
  "total": 2847,
  "page": 1,
  "per_page": 50
}`,
  },
  {
    method: "GET",
    path: "/api/primes/:id",
    description: "Get a single prime with full certificate data.",
  },
  {
    method: "GET",
    path: "/api/stats",
    description: "Aggregate statistics: total primes, candidates tested, active workers.",
    response: `{
  "total_primes": 2847,
  "candidates_tested": 14200000000,
  "active_workers": 38,
  "compute_hours": 127000,
  "search_forms": 12
}`,
  },
  {
    method: "GET",
    path: "/api/workers",
    description: "List all registered workers with status and last heartbeat.",
  },
  {
    method: "POST",
    path: "/api/workers/register",
    description: "Register a new worker with the coordinator.",
  },
  {
    method: "POST",
    path: "/api/workers/heartbeat",
    description: "Worker heartbeat with progress and status update.",
  },
  {
    method: "GET",
    path: "/api/jobs",
    description: "List search jobs with status (running, completed, paused).",
  },
  {
    method: "POST",
    path: "/api/jobs/claim",
    description: "Claim the next available work block (FOR UPDATE SKIP LOCKED).",
  },
  {
    method: "POST",
    path: "/api/jobs/:id/complete",
    description: "Mark a work block as completed with results.",
  },
  {
    method: "GET",
    path: "/api/status",
    description: "Service health check and system metrics.",
    response: `{
  "status": "healthy",
  "version": "0.1.0",
  "uptime_seconds": 86400,
  "database": "connected",
  "workers_online": 38
}`,
  },
  {
    method: "GET",
    path: "/api/volunteer/worker/latest?channel=stable",
    description:
      "Get latest downloadable worker release metadata for a channel (optionally worker-specific via worker_id for canary rollout).",
    response: `{
  "channel": "stable",
  "version": "0.1.0",
  "published_at": "2026-02-20T00:00:00Z",
  "notes": "Initial public worker release channel",
  "artifacts": [
    {
      "os": "linux",
      "arch": "x86_64",
      "url": "https://downloads.darkreach.example/worker/v0.1.0/darkreach-worker-linux-x86_64.tar.gz",
      "sha256": "...",
      "sig_url": "https://downloads.darkreach.example/worker/v0.1.0/darkreach-worker-linux-x86_64.tar.gz.sig"
    }
  ]
}`,
  },
  {
    method: "POST",
    path: "/api/releases/worker",
    description: "Upsert a worker release record in the rollout control plane.",
  },
  {
    method: "POST",
    path: "/api/releases/rollout",
    description: "Set release channel target version and rollout percent (canary/ramp).",
  },
  {
    method: "POST",
    path: "/api/releases/rollback",
    description: "Rollback a channel to the previous version in rollout history.",
  },
  {
    method: "GET",
    path: "/api/releases/events?channel=stable&limit=100",
    description: "List rollout/rollback events for audit trail.",
  },
  {
    method: "GET",
    path: "/api/releases/health?active_hours=24",
    description: "Release adoption summary by worker version and channel targets.",
  },
];

const wsEvents = [
  {
    event: "prime_discovered",
    direction: "server → client",
    description: "Broadcast when a new prime is found.",
    payload: `{ "form": "factorial", "expression": "147855! + 1", "digits": 636919 }`,
  },
  {
    event: "worker_status",
    direction: "server → client",
    description: "Fleet status update (every 30s).",
    payload: `{ "workers": [...], "active_searches": [...] }`,
  },
  {
    event: "search_progress",
    direction: "server → client",
    description: "Search progress update with candidates tested and rate.",
    payload: `{ "job_id": 1, "progress": 0.42, "candidates_per_second": 15000 }`,
  },
  {
    event: "subscribe",
    direction: "client → server",
    description: "Subscribe to specific event channels.",
    payload: `{ "channels": ["primes", "fleet", "searches"] }`,
  },
];

function MethodBadge({ method }: { method: string }) {
  const variant =
    method === "GET" ? "green" : method === "POST" ? "purple" : "default";
  return <Badge variant={variant}>{method}</Badge>;
}

export default function ApiPage() {
  return (
    <div className="prose-docs">
      <h1>API Reference</h1>
      <p>
        The darkreach coordinator exposes a REST API and WebSocket endpoint for
        workers, the dashboard, and third-party integrations.
      </p>
      <p>
        <strong>Base URL:</strong>{" "}
        <code>https://api.darkreach.ai</code>
      </p>

      <h2>REST Endpoints</h2>
      <div className="space-y-6 mt-4">
        {restEndpoints.map((ep) => (
          <div
            key={`${ep.method}-${ep.path}`}
            className="border border-border rounded-lg p-4 bg-card"
          >
            <div className="flex items-center gap-3 mb-2">
              <MethodBadge method={ep.method} />
              <code className="text-sm text-accent-purple">{ep.path}</code>
            </div>
            <p className="text-sm text-muted-foreground m-0">{ep.description}</p>
            {ep.response && (
              <div className="mt-3">
                <CodeBlock language="json">{ep.response}</CodeBlock>
              </div>
            )}
          </div>
        ))}
      </div>

      <h2>WebSocket</h2>
      <p>
        Connect to <code>wss://api.darkreach.ai/ws</code> for real-time
        updates. Messages are JSON-encoded with an <code>event</code> field.
      </p>

      <div className="space-y-6 mt-4">
        {wsEvents.map((ev) => (
          <div
            key={ev.event}
            className="border border-border rounded-lg p-4 bg-card"
          >
            <div className="flex items-center gap-3 mb-2">
              <Badge variant={ev.direction.startsWith("server") ? "green" : "purple"}>
                {ev.direction}
              </Badge>
              <code className="text-sm text-accent-purple">{ev.event}</code>
            </div>
            <p className="text-sm text-muted-foreground m-0 mb-3">
              {ev.description}
            </p>
            <CodeBlock language="json">{ev.payload}</CodeBlock>
          </div>
        ))}
      </div>

      <h2>Authentication</h2>
      <p>
        The API is currently unauthenticated. Worker registration uses
        coordinator-assigned worker IDs. Future versions will add API key
        authentication for write operations.
      </p>
    </div>
  );
}
