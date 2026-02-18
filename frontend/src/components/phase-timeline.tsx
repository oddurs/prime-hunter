/**
 * @module phase-timeline
 *
 * Vertical timeline showing the phases of a project. Each phase displays
 * its status, dependency chain, progress, and link to its search job.
 */

import { Badge } from "@/components/ui/badge";
import { numberWithCommas } from "@/lib/format";
import {
  CheckCircle2,
  Circle,
  Loader2,
  SkipForward,
  XCircle,
} from "lucide-react";

interface Phase {
  id: number;
  name: string;
  description: string;
  phase_order: number;
  status: string;
  total_tested: number;
  total_found: number;
  search_job_id: number | null;
  started_at: string | null;
  completed_at: string | null;
}

const statusIcons: Record<string, typeof Circle> = {
  pending: Circle,
  active: Loader2,
  completed: CheckCircle2,
  skipped: SkipForward,
  failed: XCircle,
};

const statusColors: Record<string, string> = {
  pending: "text-muted-foreground",
  active: "text-green-500 animate-spin",
  completed: "text-blue-500",
  skipped: "text-muted-foreground",
  failed: "text-red-500",
};

export function PhaseTimeline({ phases }: { phases: Phase[] }) {
  if (phases.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">No phases defined</p>
    );
  }

  return (
    <div className="relative">
      {phases.map((phase, idx) => {
        const Icon = statusIcons[phase.status] ?? Circle;
        const color = statusColors[phase.status] ?? "text-muted-foreground";
        const isLast = idx === phases.length - 1;

        return (
          <div key={phase.id} className="flex gap-3">
            {/* Timeline line + icon */}
            <div className="flex flex-col items-center">
              <Icon className={`h-5 w-5 ${color} flex-shrink-0`} />
              {!isLast && (
                <div className="w-px flex-1 bg-border min-h-[32px]" />
              )}
            </div>

            {/* Phase content */}
            <div className="pb-4 flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="font-medium text-sm">{phase.name}</span>
                <Badge variant="outline" className="text-xs">
                  {phase.status}
                </Badge>
                {phase.search_job_id && (
                  <span className="text-xs text-muted-foreground">
                    job #{phase.search_job_id}
                  </span>
                )}
              </div>
              {phase.description && (
                <p className="text-xs text-muted-foreground mt-0.5">
                  {phase.description}
                </p>
              )}
              {(phase.total_tested > 0 || phase.total_found > 0) && (
                <p className="text-xs text-muted-foreground mt-1">
                  {numberWithCommas(phase.total_tested)} tested,{" "}
                  {phase.total_found} found
                </p>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
