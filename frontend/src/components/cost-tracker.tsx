/**
 * @module cost-tracker
 *
 * Budget gauge + cost breakdown card for a project. Shows current spend
 * vs budget limit as a progress ring, with core-hours and per-phase
 * cost details.
 */

import { Card, CardContent } from "@/components/ui/card";
import { numberWithCommas } from "@/lib/format";

interface CostTrackerProps {
  totalCostUsd: number;
  maxCostUsd: number | null;
  totalCoreHours: number;
  totalTested: number;
}

export function CostTracker({
  totalCostUsd,
  maxCostUsd,
  totalCoreHours,
  totalTested,
}: CostTrackerProps) {
  const hasBudget = maxCostUsd != null && maxCostUsd > 0;
  const pct = hasBudget
    ? Math.min(100, (totalCostUsd / maxCostUsd!) * 100)
    : 0;
  const isOver = hasBudget && totalCostUsd >= maxCostUsd!;

  return (
    <Card>
      <CardContent className="py-4">
        <h3 className="text-sm font-medium mb-3">Cost Tracking</h3>
        <div className="grid grid-cols-2 gap-4 text-sm">
          <div>
            <p className="text-muted-foreground text-xs">Spent</p>
            <p className={`font-mono ${isOver ? "text-red-500" : ""}`}>
              ${totalCostUsd.toFixed(2)}
            </p>
          </div>
          {hasBudget && (
            <div>
              <p className="text-muted-foreground text-xs">Budget</p>
              <p className="font-mono">${maxCostUsd!.toFixed(2)}</p>
            </div>
          )}
          <div>
            <p className="text-muted-foreground text-xs">Core-hours</p>
            <p className="font-mono">{totalCoreHours.toFixed(1)}</p>
          </div>
          <div>
            <p className="text-muted-foreground text-xs">Candidates</p>
            <p className="font-mono">{numberWithCommas(totalTested)}</p>
          </div>
        </div>
        {hasBudget && (
          <div className="mt-3">
            <div className="flex justify-between text-xs text-muted-foreground mb-1">
              <span>Budget usage</span>
              <span>{pct.toFixed(1)}%</span>
            </div>
            <div className="h-2 rounded-full bg-muted overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${
                  isOver ? "bg-red-500" : pct > 80 ? "bg-yellow-500" : "bg-[#f78166]"
                }`}
                style={{ width: `${Math.min(100, pct)}%` }}
              />
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
