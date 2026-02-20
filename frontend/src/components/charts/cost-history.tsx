/**
 * @module cost-history
 *
 * Cumulative cost time-series chart for a project. Displays core-hours
 * or USD spent over time as a filled area chart, with an optional
 * horizontal budget limit line.
 *
 * Uses Recharts AreaChart with the same conventions as discovery-timeline
 * and digit-distribution charts.
 */

"use client";

import { useCallback, useEffect, useState } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ReferenceLine,
  ResponsiveContainer,
} from "recharts";

interface CostHistoryPoint {
  time: string;
  core_hours: number;
  cost_usd: number;
}

interface CostHistoryData {
  history: CostHistoryPoint[];
  budget_usd: number | null;
  cloud_rate: number;
  total_core_hours: number;
  total_cost_usd: number;
}

interface CostHistoryChartProps {
  slug: string;
  apiBase: string;
}

/** Format an ISO timestamp as a short date/time label. */
function formatTime(iso: string): string {
  const d = new Date(iso);
  const now = new Date();
  const diffDays = (now.getTime() - d.getTime()) / (1000 * 60 * 60 * 24);
  if (diffDays < 1) {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

export function CostHistoryChart({ slug, apiBase }: CostHistoryChartProps) {
  const [data, setData] = useState<CostHistoryData | null>(null);

  const fetchHistory = useCallback(async () => {
    try {
      const res = await fetch(`${apiBase}/api/projects/${slug}/cost-history`);
      if (res.ok) {
        const json = await res.json();
        setData(json);
      }
    } catch {
      // Silently ignore fetch errors
    }
  }, [slug, apiBase]);

  useEffect(() => {
    fetchHistory();
    const interval = setInterval(fetchHistory, 30_000); // Refresh every 30s
    return () => clearInterval(interval);
  }, [fetchHistory]);

  if (!data || data.history.length === 0) {
    return (
      <div className="flex items-center justify-center h-[160px] text-xs text-muted-foreground">
        No cost data yet
      </div>
    );
  }

  const chartData = data.history.map((p) => ({
    time: formatTime(p.time),
    rawTime: p.time,
    cost: p.cost_usd,
    coreHours: p.core_hours,
  }));

  const maxCost = Math.max(
    ...chartData.map((d) => d.cost),
    data.budget_usd ?? 0
  );

  return (
    <div>
      <div className="flex items-center justify-between mb-1">
        <span className="text-xs text-muted-foreground">Cumulative cost</span>
        <span className="text-xs font-mono tabular-nums">
          ${data.total_cost_usd.toFixed(2)}
        </span>
      </div>
      <ResponsiveContainer width="100%" height={160}>
        <AreaChart data={chartData}>
          <XAxis
            dataKey="time"
            tick={{ fontSize: 10 }}
            interval="preserveStartEnd"
          />
          <YAxis
            tick={{ fontSize: 10 }}
            width={50}
            tickFormatter={(v: number) => `$${v.toFixed(v >= 10 ? 0 : 2)}`}
            domain={[0, Math.ceil(maxCost * 1.1) || 1]}
          />
          <Tooltip
            contentStyle={{
              fontSize: 12,
              background: "var(--popover)",
              border: "1px solid var(--border)",
              borderRadius: 6,
            }}
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            formatter={(value: any, name: any) => {
              const v = Number(value);
              if (name === "cost") return [`$${v.toFixed(2)}`, "Cost"];
              return [`${v.toFixed(1)} hrs`, "Core-hours"];
            }}
            labelFormatter={(label: any) => String(label)}
          />
          <Area
            type="monotone"
            dataKey="cost"
            stroke="var(--chart-1)"
            fill="var(--chart-1)"
            fillOpacity={0.15}
            strokeWidth={1.5}
            dot={false}
            isAnimationActive={false}
          />
          {data.budget_usd != null && data.budget_usd > 0 && (
            <ReferenceLine
              y={data.budget_usd}
              stroke="var(--chart-5)"
              strokeDasharray="4 4"
              strokeWidth={1}
              label={{
                value: `Budget $${data.budget_usd.toFixed(0)}`,
                position: "right",
                fontSize: 10,
                fill: "var(--chart-5)",
              }}
            />
          )}
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
