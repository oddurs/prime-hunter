/**
 * @module agents/analytics-tab
 *
 * Agent cost analytics: daily cost trend chart (stacked by model),
 * template efficiency table, and token anomaly detection (>3x average).
 */

import { Card, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  useAgentDailyCosts,
  useAgentTemplateCosts,
  useAgentAnomalies,
  type DailyCostRow,
} from "@/hooks/use-agents";
import { BarChart3, LayoutTemplate, AlertCircle } from "lucide-react";
import { numberWithCommas } from "@/lib/format";

/** Simple daily cost bar chart using Recharts. */
function DailyCostChart({ data }: { data: DailyCostRow[] }) {
  // Pivot by date — sum costs per model
  const byDate = new Map<string, Record<string, number>>();
  for (const row of data) {
    const existing = byDate.get(row.date) ?? {};
    existing[row.model] = (existing[row.model] ?? 0) + row.total_cost;
    byDate.set(row.date, existing);
  }
  const models = [...new Set(data.map((d) => d.model))];
  const chartData = [...byDate.entries()].map(([date, costs]) => ({
    date: date.slice(5), // MM-DD
    ...costs,
  }));

  const colors = ["hsl(var(--primary))", "hsl(var(--chart-2))", "hsl(var(--chart-3))"];

  // Dynamic import to avoid SSR issues
  const {
    BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Legend,
  } = require("recharts");

  return (
    <div className="h-48">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={chartData}>
          <XAxis dataKey="date" tick={{ fontSize: 10 }} />
          <YAxis tick={{ fontSize: 10 }} tickFormatter={(v: number) => `$${v.toFixed(2)}`} />
          <Tooltip
            formatter={(value: number) => `$${Number(value).toFixed(4)}`}
            contentStyle={{ fontSize: 11 }}
          />
          <Legend wrapperStyle={{ fontSize: 11 }} />
          {models.map((model, i) => (
            <Bar
              key={model}
              dataKey={model}
              stackId="cost"
              fill={colors[i % colors.length]}
            />
          ))}
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

export function AnalyticsTab() {
  const { data: dailyCosts, loading: loadingDaily } = useAgentDailyCosts(30);
  const { data: templateCosts, loading: loadingTemplate } = useAgentTemplateCosts();
  const { data: anomalies, loading: loadingAnomalies } = useAgentAnomalies(3);

  return (
    <div className="space-y-6">
      {/* Daily Cost Trend */}
      <Card>
        <CardContent className="p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <BarChart3 className="size-4" />
            Daily Cost (Last 30 Days)
          </h3>
          {loadingDaily ? (
            <div className="text-xs text-muted-foreground py-8 text-center">Loading...</div>
          ) : dailyCosts.length === 0 ? (
            <div className="text-xs text-muted-foreground py-8 text-center">No cost data yet</div>
          ) : (
            <DailyCostChart data={dailyCosts} />
          )}
        </CardContent>
      </Card>

      {/* Template Efficiency */}
      <Card>
        <CardContent className="p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <LayoutTemplate className="size-4" />
            Template Efficiency
          </h3>
          {loadingTemplate ? (
            <div className="text-xs text-muted-foreground py-8 text-center">Loading...</div>
          ) : templateCosts.length === 0 ? (
            <div className="text-xs text-muted-foreground py-8 text-center">
              No template data yet
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Template</TableHead>
                  <TableHead className="text-right">Tasks</TableHead>
                  <TableHead className="text-right">Total Cost</TableHead>
                  <TableHead className="text-right">Avg Cost</TableHead>
                  <TableHead className="text-right">Avg Tokens</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {templateCosts.map((t) => (
                  <TableRow key={t.template_name}>
                    <TableCell className="font-mono text-xs">{t.template_name}</TableCell>
                    <TableCell className="text-right tabular-nums">{t.task_count}</TableCell>
                    <TableCell className="text-right tabular-nums">${t.total_cost.toFixed(4)}</TableCell>
                    <TableCell className="text-right tabular-nums">${t.avg_cost.toFixed(4)}</TableCell>
                    <TableCell className="text-right tabular-nums">
                      {numberWithCommas(Math.round(t.avg_tokens))}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      {/* Anomalies */}
      <Card>
        <CardContent className="p-4">
          <h3 className="text-sm font-medium mb-3 flex items-center gap-2">
            <AlertCircle className="size-4 text-amber-500" />
            Token Anomalies ({">"}3x avg)
          </h3>
          {loadingAnomalies ? (
            <div className="text-xs text-muted-foreground py-8 text-center">Loading...</div>
          ) : anomalies.length === 0 ? (
            <div className="text-xs text-muted-foreground py-8 text-center">
              No anomalies detected
            </div>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Task</TableHead>
                  <TableHead>Template</TableHead>
                  <TableHead className="text-right">Tokens</TableHead>
                  <TableHead className="text-right">Cost</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {anomalies.map((t) => (
                  <TableRow key={t.id}>
                    <TableCell>
                      <div className="text-xs font-medium">{t.title}</div>
                      <div className="text-[10px] text-muted-foreground">#{t.id}</div>
                    </TableCell>
                    <TableCell className="font-mono text-xs">{t.template_name ?? "-"}</TableCell>
                    <TableCell className="text-right tabular-nums text-amber-500 font-medium">
                      {numberWithCommas(t.tokens_used)}
                    </TableCell>
                    <TableCell className="text-right tabular-nums">${t.cost_usd.toFixed(4)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
