/**
 * @module phase-timeline
 *
 * Vertical timeline showing the phases of a project. Each phase displays
 * its status, dependency chain, progress, and link to its search job.
 * Dependency arrows show which phases must complete before others can start.
 */

import { Badge } from "@/components/ui/badge";
import { numberWithCommas } from "@/lib/format";
import {
  ArrowDown,
  CheckCircle2,
  Circle,
  Clock,
  Loader2,
  Lock,
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
  depends_on?: string[];
  activation_condition?: string | null;
  completion_condition?: string;
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

/** Format an activation condition into a readable string. */
function formatCondition(condition: string): string {
  if (condition === "previous_phase_found_zero")
    return "Previous phase found no primes";
  if (condition === "previous_phase_found_prime")
    return "Previous phase found a prime";
  if (condition.startsWith("time_since_start:")) {
    const hours = condition.split(":")[1];
    return `${hours}h after project start`;
  }
  return condition;
}

/** Describe why a pending phase is blocked. */
function getBlockingReason(phase: Phase, allPhases: Phase[]): string | null {
  if (phase.status !== "pending") return null;

  const reasons: string[] = [];

  // Check unmet dependencies
  if (phase.depends_on && phase.depends_on.length > 0) {
    const unmet = phase.depends_on.filter((depName) => {
      const dep = allPhases.find((p) => p.name === depName);
      return dep && dep.status !== "completed" && dep.status !== "skipped";
    });
    if (unmet.length > 0) {
      reasons.push(`Waiting for: ${unmet.join(", ")}`);
    }
  }

  // Check activation condition
  if (phase.activation_condition) {
    reasons.push(formatCondition(phase.activation_condition));
  }

  return reasons.length > 0 ? reasons.join(" + ") : null;
}

export function PhaseTimeline({ phases }: { phases: Phase[] }) {
  if (phases.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">No phases defined</p>
    );
  }

  // Build a name→index map for dependency arrow rendering
  const nameToIdx = new Map(phases.map((p, i) => [p.name, i]));

  return (
    <div className="relative">
      {phases.map((phase, idx) => {
        const Icon = statusIcons[phase.status] ?? Circle;
        const color = statusColors[phase.status] ?? "text-muted-foreground";
        const isLast = idx === phases.length - 1;
        const blockReason = getBlockingReason(phase, phases);

        // Find non-adjacent dependency arrows (deps not immediately above)
        const depArrows: string[] = (phase.depends_on ?? []).filter(
          (dep) => {
            const depIdx = nameToIdx.get(dep);
            return depIdx !== undefined && depIdx !== idx - 1;
          }
        );

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
              <div className="flex items-center gap-2 flex-wrap">
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

              {/* Dependency badges */}
              {phase.depends_on && phase.depends_on.length > 0 && (
                <div className="flex items-center gap-1 mt-1 flex-wrap">
                  <ArrowDown className="h-3 w-3 text-muted-foreground flex-shrink-0" />
                  {phase.depends_on.map((dep) => {
                    const depPhase = phases.find((p) => p.name === dep);
                    const resolved =
                      depPhase?.status === "completed" ||
                      depPhase?.status === "skipped";
                    return (
                      <Badge
                        key={dep}
                        variant={resolved ? "secondary" : "outline"}
                        className={`text-[10px] ${resolved ? "opacity-60" : ""}`}
                      >
                        {resolved ? (
                          <CheckCircle2 className="h-2.5 w-2.5 mr-0.5" />
                        ) : (
                          <Lock className="h-2.5 w-2.5 mr-0.5" />
                        )}
                        {dep}
                      </Badge>
                    );
                  })}
                </div>
              )}

              {/* Blocking reason for pending phases */}
              {blockReason && (
                <p className="text-[10px] text-amber-500 dark:text-amber-400 mt-1 flex items-center gap-1">
                  <Clock className="h-3 w-3 flex-shrink-0" />
                  {blockReason}
                </p>
              )}

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

              {/* Completion condition hint */}
              {phase.completion_condition &&
                phase.completion_condition !== "all_blocks_done" && (
                  <p className="text-[10px] text-muted-foreground mt-0.5 italic">
                    Completes: {phase.completion_condition}
                  </p>
                )}

              {/* Non-adjacent dependency arrows label */}
              {depArrows.length > 0 && (
                <p className="text-[10px] text-muted-foreground mt-0.5">
                  (depends on non-adjacent: {depArrows.join(", ")})
                </p>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
