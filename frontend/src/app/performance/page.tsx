"use client";

/**
 * @module performance/page
 *
 * Grafana-style observability view for coordinator + fleet metrics,
 * long-term trends, and stored system logs.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  LineChart,
  Line,
  ResponsiveContainer,
  XAxis,
  YAxis,
  Tooltip,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ViewHeader } from "@/components/view-header";
import { useWs } from "@/contexts/websocket-context";
import { API_BASE, formatTime, numberWithCommas, relativeTime } from "@/lib/format";
import { StatCard } from "@/components/stat-card";

const ranges = [
  { label: "6h", hours: 6 },
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];

interface MetricPoint {
  ts: string;
  value: number;
}

interface MetricSeriesResponse {
  metric: string;
  scope?: string;
  worker_id?: string;
  points: MetricPoint[];
}

interface LogsResponse {
  logs: Array<{
    id: number;
    ts: string;
    level: string;
    source: string;
    component: string;
    message: string;
    worker_id?: string | null;
    context?: Record<string, unknown> | null;
  }>;
}

interface ReportResponse {
  from: string;
  to: string;
  primes: { total: number; by_form: Array<[string, number]> };
  logs: { by_level: Array<[string, number]> };
  fleet: {
    workers_peak?: number | null;
    tested_delta?: number | null;
    found_delta?: number | null;
  };
  coordinator: {
    avg_cpu_usage_percent?: number | null;
  };
}

function toIsoRange(hours: number) {
  const to = new Date();
  const from = new Date(Date.now() - hours * 3600 * 1000);
  return { from: from.toISOString(), to: to.toISOString() };
}

async function fetchMetrics(
  metrics: string[],
  scope: string,
  from: string,
  to: string,
  worker_id?: string
) {
  const params = new URLSearchParams({
    metrics: metrics.join(","),
    scope,
    from,
    to,
  });
  if (worker_id) params.set("worker_id", worker_id);
  const res = await fetch(`${API_BASE}/api/observability/metrics?${params}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const data = (await res.json()) as { series: MetricSeriesResponse[] };
  return data.series;
}

async function fetchLogs(from: string, to: string) {
  const params = new URLSearchParams({ from, to, limit: "200" });
  const res = await fetch(`${API_BASE}/api/observability/logs?${params}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()) as LogsResponse;
}

async function fetchReport(from: string, to: string) {
  const params = new URLSearchParams({ from, to });
  const res = await fetch(`${API_BASE}/api/observability/report?${params}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return (await res.json()) as ReportResponse;
}

function buildRateSeries(points: MetricPoint[]) {
  const out: Array<{ ts: string; rate: number }> = [];
  for (let i = 1; i < points.length; i += 1) {
    const prev = points[i - 1];
    const curr = points[i];
    const dt = (new Date(curr.ts).getTime() - new Date(prev.ts).getTime()) / 1000;
    if (dt <= 0) continue;
    out.push({ ts: curr.ts, rate: (curr.value - prev.value) / dt });
  }
  return out;
}

export default function PerformancePage() {
  const { fleet, coordinator, connected } = useWs();
  const [range, setRange] = useState(ranges[0]);
  const [series, setSeries] = useState<MetricSeriesResponse[]>([]);
  const [logs, setLogs] = useState<LogsResponse["logs"]>([]);
  const [report, setReport] = useState<ReportResponse | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    const { from, to } = toIsoRange(range.hours);
    setLoading(true);
    try {
      const metrics = await fetchMetrics(
        [
          "fleet.total_tested",
          "fleet.workers_connected",
          "fleet.work_blocks_available",
          "fleet.work_blocks_claimed",
          "fleet.search_jobs_active",
        ],
        "fleet",
        from,
        to
      );
      const coordMetrics = await fetchMetrics(
        ["coordinator.cpu_usage_percent", "coordinator.memory_usage_percent"],
        "coordinator",
        from,
        to
      );
      setSeries([...metrics, ...coordMetrics]);

      const logsResp = await fetchLogs(from, to);
      setLogs(logsResp.logs ?? []);

      const reportResp = await fetchReport(from, to);
      setReport(reportResp);
    } catch {
      setSeries([]);
      setLogs([]);
      setReport(null);
    } finally {
      setLoading(false);
    }
  }, [range]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const throughputSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "fleet.total_tested");
    return s ? buildRateSeries(s.points) : [];
  }, [series]);

  const workersSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "fleet.workers_connected");
    return s ? s.points : [];
  }, [series]);

  const workBlocksSeries = useMemo(() => {
    const available = series.find((m) => m.metric === "fleet.work_blocks_available");
    const claimed = series.find((m) => m.metric === "fleet.work_blocks_claimed");
    const merged: Array<{ ts: string; available: number; claimed: number }> = [];
    if (!available || !claimed) return merged;
    const map = new Map<string, { available?: number; claimed?: number }>();
    for (const p of available.points) {
      map.set(p.ts, { available: p.value });
    }
    for (const p of claimed.points) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, claimed: p.value });
    }
    for (const [ts, vals] of map.entries()) {
      merged.push({
        ts,
        available: vals.available ?? 0,
        claimed: vals.claimed ?? 0,
      });
    }
    merged.sort((a, b) => new Date(a.ts).getTime() - new Date(b.ts).getTime());
    return merged;
  }, [series]);

  const searchJobsSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "fleet.search_jobs_active");
    return s ? s.points : [];
  }, [series]);

  const coordCpuSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "coordinator.cpu_usage_percent");
    return s ? s.points : [];
  }, [series]);

  const coordMemSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "coordinator.memory_usage_percent");
    return s ? s.points : [];
  }, [series]);

  const primeByForm = report?.primes.by_form ?? [];
  const logByLevel = report?.logs.by_level ?? [];

  const onDownloadReport = useCallback(() => {
    if (!report) return;
    const blob = new Blob([JSON.stringify(report, null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `observability-report-${new Date().toISOString()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [report]);

  return (
    <>
      <ViewHeader
        title="Observability"
        subtitle="Grafana-style metrics, long-term trends, and system logs"
        metadata={
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline">{connected ? "Live" : "Offline"}</Badge>
            <Badge variant="outline">
              {fleet?.total_workers ?? 0} workers · {fleet?.total_cores ?? 0} cores
            </Badge>
            {coordinator && (
              <Badge variant="outline">
                Coordinator CPU {Math.round(coordinator.cpu_usage_percent)}%
              </Badge>
            )}
          </div>
        }
        actions={
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-1">
              {ranges.map((r) => (
                <Button
                  key={r.label}
                  size="sm"
                  variant={r.label === range.label ? "default" : "outline"}
                  onClick={() => setRange(r)}
                >
                  {r.label}
                </Button>
              ))}
            </div>
            <Button size="sm" variant="outline" onClick={refresh}>
              Refresh
            </Button>
            <Button size="sm" onClick={onDownloadReport} disabled={!report}>
              Export Report
            </Button>
          </div>
        }
      />

      <div className="grid grid-cols-1 lg:grid-cols-4 gap-4 mb-6">
        <StatCard
          label="Primes found"
          value={numberWithCommas(report?.primes.total ?? 0)}
        />
        <StatCard
          label="Tested (range)"
          value={numberWithCommas(Math.max(0, Math.floor(report?.fleet.tested_delta ?? 0)))}
        />
        <StatCard
          label="Peak workers"
          value={numberWithCommas(Math.floor(report?.fleet.workers_peak ?? 0))}
        />
        <StatCard
          label="Errors"
          value={numberWithCommas(logByLevel.find(([lvl]) => lvl === "error")?.[1] ?? 0)}
        />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Throughput (candidates/sec)
            </CardTitle>
          </CardHeader>
          <CardContent className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={throughputSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis tickFormatter={(v) => numberWithCommas(Math.round(v))} />
                <Tooltip
                  formatter={(v: number | undefined) => [numberWithCommas(Math.round(v ?? 0)), "c/s"]}
                  labelFormatter={(v) => formatTime(v)}
                />
                <Line type="monotone" dataKey="rate" stroke="#f78166" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Worker count
            </CardTitle>
          </CardHeader>
          <CardContent className="h-56">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={workersSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis allowDecimals={false} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="value" stroke="#7dd3fc" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Work blocks
            </CardTitle>
          </CardHeader>
          <CardContent className="h-52">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={workBlocksSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis allowDecimals={false} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="available" stroke="#86efac" dot={false} />
                <Line type="monotone" dataKey="claimed" stroke="#fbbf24" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Active search jobs
            </CardTitle>
          </CardHeader>
          <CardContent className="h-52">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={searchJobsSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis allowDecimals={false} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="step" dataKey="value" stroke="#c084fc" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Coordinator load
            </CardTitle>
          </CardHeader>
          <CardContent className="h-52">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={coordCpuSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis domain={[0, 100]} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="value" stroke="#f472b6" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Coordinator memory
            </CardTitle>
          </CardHeader>
          <CardContent className="h-48">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={coordMemSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis domain={[0, 100]} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="value" stroke="#38bdf8" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Prime mix (range)
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {primeByForm.length === 0 && (
              <div className="text-sm text-muted-foreground">No primes in range.</div>
            )}
            {primeByForm.map(([form, count]) => (
              <div key={form} className="flex items-center justify-between text-sm">
                <span className="capitalize">{form.replace(/_/g, " ")}</span>
                <span className="tabular-nums font-medium">{numberWithCommas(count)}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Recent logs
            </CardTitle>
          </CardHeader>
          <CardContent className="max-h-[360px] overflow-y-auto">
            {logs.length === 0 ? (
              <div className="text-sm text-muted-foreground">No logs in range.</div>
            ) : (
              <div className="space-y-2">
                {logs.map((log) => (
                  <div
                    key={log.id}
                    className="rounded-md border border-border/60 p-2 text-xs"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-medium">
                        {log.level.toUpperCase()} · {log.component}
                      </span>
                      <span className="text-muted-foreground">
                        {relativeTime(log.ts)}
                      </span>
                    </div>
                    <div className="text-muted-foreground">{log.message}</div>
                    {log.worker_id && (
                      <div className="text-muted-foreground">
                        worker: {log.worker_id}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Log levels
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            {logByLevel.length === 0 && (
              <div className="text-sm text-muted-foreground">No logs in range.</div>
            )}
            {logByLevel.map(([level, count]) => (
              <div key={level} className="flex items-center justify-between text-sm">
                <span className="uppercase">{level}</span>
                <span className="tabular-nums font-medium">{count}</span>
              </div>
            ))}
            {loading && (
              <div className="text-xs text-muted-foreground">Refreshing...</div>
            )}
          </CardContent>
        </Card>
      </div>
    </>
  );
}
