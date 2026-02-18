"use client";

/**
 * @module throughput-gauge
 *
 * Real-time throughput sparkline chart. Displays candidates tested per
 * second across the fleet as a rolling line chart (last 60 data points).
 * Data comes from the WebSocket fleet heartbeat, not Supabase.
 *
 * Uses Recharts `LineChart` in a compact card layout with no axes —
 * just the sparkline and current rate displayed as a large number.
 */

import { useEffect, useRef, useState } from "react";
import {
  LineChart,
  Line,
  ResponsiveContainer,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { FleetData } from "@/hooks/use-websocket";

function numberWithCommas(x: number): string {
  return x.toString().replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

interface Props {
  fleet: FleetData;
}

const MAX_SAMPLES = 30;

export function ThroughputGauge({ fleet }: Props) {
  const prevRef = useRef<{ tested: number; time: number } | null>(null);
  const [rate, setRate] = useState(0);
  const [history, setHistory] = useState<{ value: number }[]>([]);

  useEffect(() => {
    const now = Date.now();
    const prev = prevRef.current;

    if (prev && fleet.total_tested >= prev.tested) {
      const dtSec = (now - prev.time) / 1000;
      if (dtSec > 0.5) {
        const r = Math.round((fleet.total_tested - prev.tested) / dtSec);
        setRate(r);
        setHistory((h) => {
          const next = [...h, { value: r }];
          return next.length > MAX_SAMPLES ? next.slice(-MAX_SAMPLES) : next;
        });
      }
    }

    prevRef.current = { tested: fleet.total_tested, time: now };
  }, [fleet.total_tested]);

  if (fleet.total_workers === 0) return null;

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-xs font-medium text-muted-foreground">
          Search throughput
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex items-end gap-6">
          <div>
            <div className="text-3xl font-semibold text-foreground tabular-nums">
              {numberWithCommas(rate)}
            </div>
            <div className="text-sm text-muted-foreground">candidates/sec</div>
          </div>
          {history.length > 1 && (
            <div className="flex-1 h-[50px]">
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
  );
}
