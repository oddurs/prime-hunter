"use client";

/**
 * @module search-job-card
 *
 * Card component for displaying a PostgreSQL-backed search job.
 * These jobs use block-based work distribution — workers claim blocks
 * via `claim_work_block()` and report results. Shows job status,
 * search parameters, block progress, and provides pause/resume/cancel
 * controls via the REST API.
 *
 * Data flow: WebSocket → `searchJobs[]` → this component.
 * Actions: `POST /api/search_jobs/{id}/pause|resume|cancel`.
 */

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { API_BASE, numberWithCommas, formLabels, relativeTime } from "@/lib/format";
import type { SearchJob } from "@/hooks/use-websocket";
import { Pause, Play, X } from "lucide-react";

function statusColor(status: SearchJob["status"]): string {
  switch (status) {
    case "running":
      return "bg-green-500";
    case "paused":
      return "bg-yellow-500";
    case "completed":
      return "bg-blue-500";
    case "pending":
      return "bg-muted-foreground";
    case "cancelled":
      return "bg-muted-foreground";
    case "failed":
      return "bg-red-500";
    default:
      return "bg-muted-foreground";
  }
}

function statusLabel(status: SearchJob["status"]): string {
  return status.charAt(0).toUpperCase() + status.slice(1);
}

/** Format search job params into a human-readable string. */
function formatJobParams(job: SearchJob): string {
  const p = job.params;
  switch (job.search_type) {
    case "kbn":
      return `k=${p.k}, base=${p.base}, n=${numberWithCommas(Number(p.min_n) || 0)}..${numberWithCommas(Number(p.max_n) || 0)}`;
    case "factorial":
    case "primorial":
      return `n=${numberWithCommas(Number(p.start) || 0)}..${numberWithCommas(Number(p.end) || 0)}`;
    case "palindromic":
    case "near_repdigit":
      return `${p.base ? `base ${p.base}, ` : ""}${numberWithCommas(Number(p.min_digits) || 0)}..${numberWithCommas(Number(p.max_digits) || 0)} digits`;
    case "cullen_woodall":
    case "carol_kynea":
      return `n=${numberWithCommas(Number(p.min_n) || 0)}..${numberWithCommas(Number(p.max_n) || 0)}`;
    case "wagstaff":
      return `exp=${numberWithCommas(Number(p.min_exp) || 0)}..${numberWithCommas(Number(p.max_exp) || 0)}`;
    case "twin":
    case "sophie_germain":
      return `k=${p.k}, base=${p.base}, n=${numberWithCommas(Number(p.min_n) || 0)}..${numberWithCommas(Number(p.max_n) || 0)}`;
    case "repunit":
      return `base ${p.base}, n=${numberWithCommas(Number(p.min_n) || 0)}..${numberWithCommas(Number(p.max_n) || 0)}`;
    case "gen_fermat":
      return `exp=${p.fermat_exp}, base=${numberWithCommas(Number(p.min_base) || 0)}..${numberWithCommas(Number(p.max_base) || 0)}`;
    default:
      return JSON.stringify(p);
  }
}

export function SearchJobCard({ job }: { job: SearchJob }) {
  const isRunning = job.status === "running";
  const isPaused = job.status === "paused";
  const isActive = isRunning || isPaused;

  async function handlePause() {
    try {
      const res = await fetch(`${API_BASE}/api/search_jobs/${job.id}/pause`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to pause job:", data.error);
      }
    } catch (e) {
      console.error("Failed to pause job:", e);
    }
  }

  async function handleResume() {
    try {
      const res = await fetch(`${API_BASE}/api/search_jobs/${job.id}/resume`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to resume job:", data.error);
      }
    } catch (e) {
      console.error("Failed to resume job:", e);
    }
  }

  async function handleCancel() {
    try {
      const res = await fetch(`${API_BASE}/api/search_jobs/${job.id}/cancel`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        console.error("Failed to cancel job:", data.error);
      }
    } catch (e) {
      console.error("Failed to cancel job:", e);
    }
  }

  /** Compute block progress from range and block size. */
  const totalBlocks = Math.ceil(
    (job.range_end - job.range_start) / Math.max(job.block_size, 1)
  );

  return (
    <Card className="py-0 border">
      <CardContent className="p-3 space-y-2">
        {/* Header: status dot, form badge, status label, ID, actions */}
        <div className="flex items-start justify-between gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <div
              className={`w-2 h-2 rounded-full flex-shrink-0 ${statusColor(job.status)}`}
            />
            <Badge variant="outline" className="text-xs">
              {formLabels[job.search_type] || job.search_type}
            </Badge>
            <span className="text-xs text-muted-foreground truncate">
              {statusLabel(job.status)}
            </span>
            <span className="text-xs text-muted-foreground">
              Job #{job.id}
            </span>
          </div>
          {isActive && (
            <div className="flex items-center gap-1">
              {isRunning && (
                <Button variant="outline" size="xs" onClick={handlePause}>
                  <Pause className="size-3" />
                  Pause
                </Button>
              )}
              {isPaused && (
                <Button variant="outline" size="xs" onClick={handleResume}>
                  <Play className="size-3" />
                  Resume
                </Button>
              )}
              <Button
                variant="outline"
                size="xs"
                className="text-red-600 hover:text-red-700"
                onClick={handleCancel}
              >
                <X className="size-3" />
                Cancel
              </Button>
            </div>
          )}
        </div>

        {/* Parameters */}
        <div className="text-xs text-muted-foreground truncate">
          {formatJobParams(job)}
        </div>

        {/* Progress stats */}
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
          <span>{numberWithCommas(job.total_found)} found</span>
          <span>{numberWithCommas(job.total_tested)} tested</span>
          <span>{numberWithCommas(totalBlocks)} blocks</span>
          <span>
            range {numberWithCommas(job.range_start)}..
            {numberWithCommas(job.range_end)}
          </span>
          <span>created {relativeTime(job.created_at)}</span>
          {job.stopped_at && (
            <span>ended {relativeTime(job.stopped_at)}</span>
          )}
        </div>

        {/* Error message */}
        {job.error && (
          <div className="text-xs text-red-500 truncate">{job.error}</div>
        )}
      </CardContent>
    </Card>
  );
}
