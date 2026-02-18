"use client";

/**
 * @module search-card
 *
 * Card component for displaying an active or completed search process.
 * Shows search type, parameters, progress (tested/found), elapsed time,
 * and provides start/stop controls via the REST API.
 */

import { useEffect, useState } from "react";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { API_BASE, numberWithCommas, formatTime, formatUptime } from "@/lib/format";
import type { ManagedSearch } from "@/hooks/use-websocket";

function getSearchStatusLabel(status: ManagedSearch["status"]): string {
  if (status === "running") return "Running";
  if (status === "paused") return "Paused";
  if (status === "completed") return "Completed";
  if (status === "cancelled") return "Cancelled";
  if (typeof status === "object" && "failed" in status) return "Failed";
  return "Unknown";
}

function getSearchStatusColor(status: ManagedSearch["status"]): string {
  if (status === "running") return "bg-green-500";
  if (status === "paused") return "bg-yellow-500";
  if (status === "completed") return "bg-blue-500";
  if (status === "cancelled") return "bg-muted-foreground";
  return "bg-red-500";
}

function formatSearchParams(s: ManagedSearch): string {
  const p = s.params;
  if (s.search_type === "kbn") {
    return `k=${p.k}, base=${p.base}, n=${numberWithCommas(p.min_n ?? 0)}..${numberWithCommas(p.max_n ?? 0)}`;
  }
  if (s.search_type === "factorial") {
    return `n=${numberWithCommas(p.start ?? 0)}..${numberWithCommas(p.end ?? 0)}`;
  }
  if (s.search_type === "palindromic") {
    return `base ${p.base}, ${p.min_digits}..${p.max_digits} digits`;
  }
  return "";
}

export function SearchCard({ search: s }: { search: ManagedSearch }) {
  const isRunning = s.status === "running";
  const isPaused = s.status === "paused";
  const [nowMs, setNowMs] = useState(() => Date.now());

  useEffect(() => {
    if (!isRunning) return;
    const interval = setInterval(() => {
      setNowMs(Date.now());
    }, 1000);
    return () => clearInterval(interval);
  }, [isRunning]);

  const elapsed = Math.floor(
    (nowMs - new Date(s.started_at).getTime()) / 1000
  );

  async function handleStop() {
    try {
      const res = await fetch(`${API_BASE}/api/searches/${s.id}`, { method: "DELETE" });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to stop search:", data.error);
      }
    } catch (e) {
      console.error("Failed to stop search:", e);
    }
  }

  async function handlePause() {
    try {
      const res = await fetch(`${API_BASE}/api/searches/${s.id}/pause`, { method: "POST" });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to pause search:", data.error);
      }
    } catch (e) {
      console.error("Failed to pause search:", e);
    }
  }

  async function handleResume() {
    try {
      const res = await fetch(`${API_BASE}/api/searches/${s.id}/resume`, { method: "POST" });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to resume search:", data.error);
      }
    } catch (e) {
      console.error("Failed to resume search:", e);
    }
  }

  const duration = s.stopped_at
    ? Math.floor(
        (new Date(s.stopped_at).getTime() - new Date(s.started_at).getTime()) / 1000
      )
    : null;

  return (
    <Card className="py-0 border">
      <CardContent className="p-3 space-y-2">
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <div className={`w-2 h-2 rounded-full flex-shrink-0 ${getSearchStatusColor(s.status)}`} />
            <Badge variant="outline" className="text-xs">
              {s.search_type}
            </Badge>
            <span className="text-xs text-muted-foreground truncate">
              {getSearchStatusLabel(s.status)}
            </span>
            <span className="text-xs text-muted-foreground">
              #{s.id}
            </span>
          </div>
          {(isRunning || isPaused) && (
            <div className="flex items-center gap-1">
              {isRunning && (
                <Button variant="outline" size="xs" onClick={handlePause}>
                  Pause
                </Button>
              )}
              {isPaused && (
                <Button variant="outline" size="xs" onClick={handleResume}>
                  Resume
                </Button>
              )}
              <Button variant="outline" size="xs" className="text-red-600 hover:text-red-700" onClick={handleStop}>
                Cancel
              </Button>
            </div>
          )}
        </div>
        <div className="text-xs text-muted-foreground truncate">
          {formatSearchParams(s)}
        </div>
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          {(s.tested > 0 || s.found > 0) && (
            <>
              <span>{numberWithCommas(s.found)} found</span>
              <span>{numberWithCommas(s.tested)} tested</span>
            </>
          )}
          <span>started {formatTime(s.started_at)}</span>
          {isRunning && <span>up {formatUptime(elapsed)}</span>}
          {s.pid && <span>pid {s.pid}</span>}
          {!isRunning && s.stopped_at && (
            <span>ended {formatTime(s.stopped_at)}</span>
          )}
          {!isRunning && duration !== null && duration > 0 && (
            <span>ran {formatUptime(duration)}</span>
          )}
        </div>
        {typeof s.status === "object" && "failed" in s.status && (
          <div className="text-xs text-red-500 truncate">
            {s.status.failed.reason}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
