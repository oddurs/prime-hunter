"use client";

import { useMemo } from "react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DigitBucket } from "@/hooks/use-distribution";

const COLORS = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
];

interface Props {
  data: DigitBucket[];
}

export function DigitDistribution({ data }: Props) {
  const { chartData, forms } = useMemo(() => {
    if (data.length === 0) return { chartData: [], forms: [] };

    const formSet = new Set<string>();
    const bucketMap = new Map<number, Record<string, number>>();

    for (const d of data) {
      formSet.add(d.form);
      if (!bucketMap.has(d.bucket_start)) {
        bucketMap.set(d.bucket_start, {});
      }
      bucketMap.get(d.bucket_start)![d.form] = d.count;
    }

    const forms = Array.from(formSet).sort();
    const starts = Array.from(bucketMap.keys()).sort((a, b) => a - b);

    const chartData = starts.map((start) => {
      const entry: Record<string, string | number> = {
        range: `${start}-${start + 10}`,
      };
      const raw = bucketMap.get(start)!;
      for (const f of forms) {
        entry[f] = raw[f] || 0;
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
          Digit distribution
        </CardTitle>
      </CardHeader>
      <CardContent>
        <ResponsiveContainer width="100%" height={220}>
          <BarChart data={chartData}>
            <XAxis dataKey="range" tick={{ fontSize: 11 }} />
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
              <Bar
                key={form}
                dataKey={form}
                stackId="1"
                fill={COLORS[i % COLORS.length]}
              />
            ))}
          </BarChart>
        </ResponsiveContainer>
      </CardContent>
    </Card>
  );
}
