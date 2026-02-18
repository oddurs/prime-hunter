"use client";

import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { API_BASE, numberWithCommas, formatUptime } from "@/lib/format";
import type { WorkerStatus, ManagedSearch } from "@/hooks/use-websocket";

function formatWorkerParams(searchType: string, params: Record<string, unknown>): string {
  switch (searchType) {
    case "kbn":
      return `k=${params.k ?? "?"}, base=${params.base ?? "?"}, n=${numberWithCommas(Number(params.min_n ?? 0))}..${numberWithCommas(Number(params.max_n ?? 0))}`;
    case "factorial":
      return `n=${numberWithCommas(Number(params.start ?? 0))}..${numberWithCommas(Number(params.end ?? 0))}`;
    case "palindromic":
      return `base ${params.base ?? "?"}, ${params.min_digits ?? "?"}..${params.max_digits ?? "?"} digits`;
    case "near_repdigit":
      return `base=${params.base ?? "?"}, digits=${params.min_digits ?? "?"}..${params.max_digits ?? "?"}`;
    case "primorial":
      return `n=${params.start ?? "?"}..${params.end ?? "?"}`;
    case "cullen_woodall":
      return `base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    case "twin":
      return `k=${params.k ?? "?"}, base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    case "sophie_germain":
      return `k=${params.k ?? "?"}, base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    case "repunit":
      return `base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    case "gen_fermat":
      return `base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    case "wagstaff":
      return `p=${params.min_p ?? params.min_n ?? "?"}..${params.max_p ?? params.max_n ?? "?"}`;
    case "carol_kynea":
      return `base=${params.base ?? "?"}, n=${params.min_n ?? "?"}..${params.max_n ?? "?"}`;
    default:
      return Object.entries(params)
        .filter(([k]) => k !== "search_type")
        .map(([k, v]) => `${k}=${v}`)
        .join(", ");
  }
}

function healthColor(secs: number): string {
  if (secs < 30) return "bg-green-500";
  if (secs < 60) return "bg-yellow-500";
  return "bg-red-500";
}

interface ProcessRowProps {
  worker: WorkerStatus;
  search: ManagedSearch | null;
}

export function ProcessRow({ worker, search }: ProcessRowProps) {
  const [nowMs, setNowMs] = useState(() => Date.now());
  const isRunning = search?.status === "running";
  const isPaused = search?.status === "paused";

  useEffect(() => {
    if (!isRunning) return;
    const interval = setInterval(() => setNowMs(Date.now()), 1000);
    return () => clearInterval(interval);
  }, [isRunning]);

  const throughput = worker.uptime_secs > 0
    ? (worker.tested / worker.uptime_secs).toFixed(1)
    : "0.0";

  let parsedParams: Record<string, unknown> | null = null;
  try {
    parsedParams = JSON.parse(worker.search_params);
  } catch {
    // leave as null
  }

  async function handlePause() {
    if (!search) return;
    try {
      await fetch(`${API_BASE}/api/searches/${search.id}/pause`, { method: "POST" });
    } catch (e) {
      console.error("Failed to pause:", e);
    }
  }

  async function handleResume() {
    if (!search) return;
    try {
      await fetch(`${API_BASE}/api/searches/${search.id}/resume`, { method: "POST" });
    } catch (e) {
      console.error("Failed to resume:", e);
    }
  }

  async function handleCancel() {
    if (!search) return;
    try {
      await fetch(`${API_BASE}/api/searches/${search.id}`, { method: "DELETE" });
    } catch (e) {
      console.error("Failed to cancel:", e);
    }
  }

  return (
    <div className="border-l-2 border-muted-foreground/20 ml-2 pl-3 py-2 space-y-1">
      {/* Line 1: status dot + worker_id + search_type badge + uptime */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className={`size-2 rounded-full flex-shrink-0 ${healthColor(worker.last_heartbeat_secs_ago)}`} />
        <span className="text-xs font-mono text-muted-foreground">{worker.worker_id}</span>
        <Badge variant="outline" className="text-[10px] px-1.5 py-0">{worker.search_type}</Badge>
        <span className="text-xs text-muted-foreground ml-auto">up {formatUptime(worker.uptime_secs)}</span>
      </div>

      {/* Line 2: search params */}
      {parsedParams && (
        <div className="text-xs text-muted-foreground truncate">
          {formatWorkerParams(worker.search_type, parsedParams)}
        </div>
      )}

      {/* Line 3: current candidate */}
      {worker.current && (
        <div className="text-xs font-mono text-muted-foreground/70 truncate">
          {worker.current}
        </div>
      )}

      {/* Line 4: stats + controls */}
      <div className="flex items-center gap-3 flex-wrap">
        <span className="text-xs text-muted-foreground tabular-nums">
          {numberWithCommas(worker.tested)} tested
        </span>
        <span className="text-xs text-muted-foreground tabular-nums">
          {numberWithCommas(worker.found)} found
        </span>
        <span className="text-xs text-muted-foreground tabular-nums">
          {throughput}/s
        </span>
        {search && (isRunning || isPaused) && (
          <div className="flex items-center gap-1 ml-auto">
            {isRunning && (
              <Button variant="outline" size="xs" onClick={handlePause}>Pause</Button>
            )}
            {isPaused && (
              <Button variant="outline" size="xs" onClick={handleResume}>Resume</Button>
            )}
            <Button
              variant="outline"
              size="xs"
              className="text-red-600 hover:text-red-700"
              onClick={handleCancel}
            >
              Cancel
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
