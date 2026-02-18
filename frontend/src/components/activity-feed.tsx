"use client";

/**
 * @module activity-feed
 *
 * Recent discoveries compact list for the dashboard Insights section.
 * Shows the last 8 primes as compact list items with:
 * - Form badge
 * - Expression (truncated)
 * - Digit count
 * - Relative timestamp
 *
 * Data comes from the `usePrimes` hook (the default descending-by-id query
 * already gives us most recent first).
 */

import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { numberWithCommas, relativeTime, formLabels } from "@/lib/format";
import type { PrimeRecord } from "@/hooks/use-primes";

interface ActivityFeedProps {
  primes: PrimeRecord[];
}

export function ActivityFeed({ primes }: ActivityFeedProps) {
  const recent = primes.slice(0, 8);

  return (
    <Card className="py-0 h-full">
      <CardContent className="p-0">
        <div className="px-4 py-2.5 border-b">
          <h3 className="text-sm font-semibold text-foreground">
            Recent Discoveries
          </h3>
        </div>
        {recent.length === 0 ? (
          <div className="px-4 py-6 text-center text-sm text-muted-foreground">
            No primes found yet
          </div>
        ) : (
          <ul className="divide-y">
            {recent.map((p) => (
              <li key={p.id} className="px-4 py-2 hover:bg-muted/50">
                <div className="flex items-center gap-2 mb-0.5">
                  <Badge variant="outline" className="text-[10px] px-1.5 py-0 font-normal">
                    {formLabels[p.form] ?? p.form}
                  </Badge>
                  <span className="font-mono text-xs text-primary truncate flex-1">
                    {p.expression}
                  </span>
                </div>
                <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
                  <span className="tabular-nums">
                    {numberWithCommas(p.digits)} digits
                  </span>
                  <span>&middot;</span>
                  <span>{relativeTime(p.found_at)}</span>
                </div>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
