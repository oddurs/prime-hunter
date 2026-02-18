"use client";

/**
 * @module insight-cards
 *
 * Four computed observation cards for the dashboard Insights section:
 *
 * 1. **Discovery Rate** — primes/day over last 7 days with trend vs prior 7
 * 2. **Last Discovery** — ticking relative time since most recent prime
 * 3. **Fleet Throughput** — candidates/sec from WebSocket fleet data + sparkline
 * 4. **Record Prime** — largest prime found (digits + expression)
 *
 * Each card shows a primary metric, secondary context, and a Lucide icon.
 * Uses `tabular-nums` for stable numeric layout.
 */

import { useEffect, useRef, useState } from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { LineChart, Line, ResponsiveContainer } from "recharts";
import { TrendingUp, TrendingDown, Clock, Gauge, Trophy } from "lucide-react";
import { numberWithCommas, relativeTime, formLabels } from "@/lib/format";
import type { TimelineBucket } from "@/hooks/use-timeline";
import type { Stats } from "@/hooks/use-stats";
import type { FleetData } from "@/hooks/use-websocket";
import type { PrimeRecord } from "@/hooks/use-primes";

interface InsightCardsProps {
  timeline: TimelineBucket[];
  stats: Stats | null;
  fleet: FleetData | null;
  latestPrime: PrimeRecord | null;
}

const MAX_SAMPLES = 30;

function isWithinDays(bucket: string, days: number): boolean {
  const cutoff = Date.now() - days * 24 * 60 * 60 * 1000;
  return new Date(bucket).getTime() >= cutoff;
}

export function InsightCards({ timeline, stats, fleet, latestPrime }: InsightCardsProps) {
  // --- Discovery rate from timeline data ---
  const recentCount = timeline
    .filter((b) => isWithinDays(b.bucket, 7))
    .reduce((sum, b) => sum + b.count, 0);
  const rate = recentCount / 7;

  const priorCount = timeline
    .filter((b) => isWithinDays(b.bucket, 14) && !isWithinDays(b.bucket, 7))
    .reduce((sum, b) => sum + b.count, 0);
  const priorRate = priorCount / 7;
  const trend = priorRate > 0 ? ((rate - priorRate) / priorRate) * 100 : 0;

  // --- Ticking "last discovery" relative time ---
  const [lastDiscoveryText, setLastDiscoveryText] = useState(
    latestPrime ? relativeTime(latestPrime.found_at) : "-"
  );
  useEffect(() => {
    if (!latestPrime) return;
    setLastDiscoveryText(relativeTime(latestPrime.found_at));
    const interval = setInterval(() => {
      setLastDiscoveryText(relativeTime(latestPrime.found_at));
    }, 30000);
    return () => clearInterval(interval);
  }, [latestPrime]);

  // --- Fleet throughput with sparkline ---
  const prevRef = useRef<{ tested: number; time: number } | null>(null);
  const [throughputRate, setThroughputRate] = useState(0);
  const [history, setHistory] = useState<{ value: number }[]>([]);

  useEffect(() => {
    if (!fleet) return;
    const now = Date.now();
    const prev = prevRef.current;

    if (prev && fleet.total_tested >= prev.tested) {
      const dtSec = (now - prev.time) / 1000;
      if (dtSec > 0.5) {
        const r = Math.round((fleet.total_tested - prev.tested) / dtSec);
        setThroughputRate(r);
        setHistory((h) => {
          const next = [...h, { value: r }];
          return next.length > MAX_SAMPLES ? next.slice(-MAX_SAMPLES) : next;
        });
      }
    }

    prevRef.current = { tested: fleet.total_tested, time: now };
  }, [fleet?.total_tested]);

  const perCore =
    fleet && fleet.total_cores > 0
      ? Math.round(throughputRate / fleet.total_cores)
      : 0;

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4">
      {/* Discovery Rate */}
      <Card className="py-3">
        <CardContent className="p-0 px-4">
          <div className="flex items-center gap-2 mb-1">
            <TrendingUp className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[11px] font-medium text-muted-foreground">
              Discovery Rate
            </span>
          </div>
          <div className="text-xl font-semibold tabular-nums text-foreground">
            {rate.toFixed(1)}/day
          </div>
          <div className="flex items-center gap-1 mt-0.5">
            {trend !== 0 && (
              <>
                {trend > 0 ? (
                  <TrendingUp className="h-3 w-3 text-green-500" />
                ) : (
                  <TrendingDown className="h-3 w-3 text-red-500" />
                )}
                <span
                  className={`text-[11px] tabular-nums ${trend > 0 ? "text-green-500" : "text-red-500"}`}
                >
                  {trend > 0 ? "+" : ""}
                  {trend.toFixed(0)}% vs prior week
                </span>
              </>
            )}
            {trend === 0 && (
              <span className="text-[11px] text-muted-foreground">
                {recentCount} in last 7d
              </span>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Last Discovery */}
      <Card className="py-3">
        <CardContent className="p-0 px-4">
          <div className="flex items-center gap-2 mb-1">
            <Clock className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[11px] font-medium text-muted-foreground">
              Last Discovery
            </span>
          </div>
          <div className="text-xl font-semibold tabular-nums text-foreground">
            {lastDiscoveryText}
          </div>
          {latestPrime && (
            <div className="text-[11px] text-muted-foreground truncate mt-0.5">
              {latestPrime.expression}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Fleet Throughput */}
      <Card className="py-3">
        <CardContent className="p-0 px-4">
          <div className="flex items-center gap-2 mb-1">
            <Gauge className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[11px] font-medium text-muted-foreground">
              Fleet Throughput
            </span>
          </div>
          <div className="flex items-end gap-3">
            <div>
              <div className="text-xl font-semibold tabular-nums text-foreground">
                {numberWithCommas(throughputRate)}/s
              </div>
              {perCore > 0 && (
                <div className="text-[11px] text-muted-foreground tabular-nums">
                  {numberWithCommas(perCore)}/core
                </div>
              )}
            </div>
            {history.length > 1 && (
              <div className="flex-1 h-[30px]">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={history}>
                    <Line
                      type="monotone"
                      dataKey="value"
                      stroke="var(--chart-1)"
                      strokeWidth={1.5}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Record Prime */}
      <Card className="py-3">
        <CardContent className="p-0 px-4">
          <div className="flex items-center gap-2 mb-1">
            <Trophy className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[11px] font-medium text-muted-foreground">
              Record Prime
            </span>
          </div>
          <div className="text-xl font-semibold tabular-nums text-foreground">
            {stats?.largest_digits
              ? `${numberWithCommas(stats.largest_digits)} digits`
              : "-"}
          </div>
          {stats?.largest_expression && (
            <div className="text-[11px] text-muted-foreground truncate mt-0.5">
              {stats.largest_expression}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
