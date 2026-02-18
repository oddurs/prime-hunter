"use client";

/**
 * @module discovery-timeline
 *
 * Stacked area chart showing prime discoveries over time, grouped by
 * form. Each prime form gets a distinct color band. Time buckets
 * (day/week/month) are fetched from the `get_discovery_timeline`
 * Supabase RPC via the `useTimeline()` hook.
 *
 * Uses Recharts `AreaChart` with stacked areas for visual comparison
 * of discovery rates across different prime forms.
 */

import { useMemo } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { TimelineBucket } from "@/hooks/use-timeline";

const COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
];

interface Props {
  data: TimelineBucket[];
}

export function DiscoveryTimeline({ data }: Props) {
  const { chartData, forms } = useMemo(() => {
    if (data.length === 0) return { chartData: [], forms: [] };

    const formSet = new Set<string>();
    const bucketMap = new Map<string, Record<string, number>>();

    for (const d of data) {
      formSet.add(d.form);
      if (!bucketMap.has(d.bucket)) {
        bucketMap.set(d.bucket, {});
      }
      bucketMap.get(d.bucket)![d.form] = d.count;
    }

    const forms = Array.from(formSet).sort();
    const buckets = Array.from(bucketMap.keys()).sort();

    // Compute cumulative sums
    const cumulative: Record<string, number> = {};
    for (const f of forms) cumulative[f] = 0;

    const chartData = buckets.map((bucket) => {
      const entry: Record<string, string | number> = { bucket };
      const raw = bucketMap.get(bucket)!;
      for (const f of forms) {
        cumulative[f] += raw[f] || 0;
        entry[f] = cumulative[f];
      }
      return entry;
    });

    return { chartData, forms };
  }, [data]);

  if (chartData.length === 0) return null;

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-xs font-medium text-muted-foreground">
          Discovery timeline
        </CardTitle>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={220}>
          <AreaChart data={chartData}>
            <XAxis
              dataKey="bucket"
              tick={{ fontSize: 11 }}
              tickFormatter={(v: string) =>
                v.length > 10 ? v.slice(5, 10) : v.slice(5)
              }
            />
            <YAxis tick={{ fontSize: 11 }} width={40} />
            <Tooltip
              contentStyle={{
                fontSize: 12,
                background: "var(--popover)",
                border: "1px solid var(--border)",
                borderRadius: 6,
              }}
            />
            {forms.map((form, i) => (
              <Area
                key={form}
                type="monotone"
                dataKey={form}
                stackId="1"
                stroke={COLORS[i % COLORS.length]}
                fill={COLORS[i % COLORS.length]}
                fillOpacity={0.4}
              />
            ))}
          </AreaChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
