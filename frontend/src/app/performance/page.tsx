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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Input } from "@/components/ui/input";

const ranges = [
  { label: "6h", hours: 6 },
  { label: "24h", hours: 24 },
  { label: "7d", hours: 24 * 7 },
  { label: "30d", hours: 24 * 30 },
];
const topWorkerWindows = [
  { label: "5m", minutes: 5 },
  { label: "30m", minutes: 30 },
  { label: "2h", minutes: 120 },
];

const metricsCatalog = [
  { key: "fleet.total_tested", label: "Fleet tested" },
  { key: "fleet.workers_connected", label: "Fleet workers" },
  { key: "fleet.work_blocks_available", label: "Blocks available" },
  { key: "fleet.work_blocks_claimed", label: "Blocks claimed" },
  { key: "fleet.work_blocks_completed", label: "Blocks completed" },
  { key: "fleet.work_blocks_failed", label: "Blocks failed" },
  { key: "fleet.search_jobs_active", label: "Active jobs" },
  { key: "fleet.max_heartbeat_age_secs", label: "Max heartbeat age" },
  { key: "coordinator.cpu_usage_percent", label: "Coord CPU" },
  { key: "coordinator.memory_usage_percent", label: "Coord mem" },
  { key: "coordinator.tick_drift_ms", label: "Tick drift" },
  { key: "events.error_count", label: "Errors" },
  { key: "events.warning_count", label: "Warnings" },
];

const ERROR_BUDGET_ERRORS_PER_HOUR =
  Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_ERRORS_PER_HOUR) || 10;
const ERROR_BUDGET_WARNINGS_PER_HOUR =
  Number(process.env.NEXT_PUBLIC_ERROR_BUDGET_WARNINGS_PER_HOUR) || 50;

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
  duration_hours?: number;
  primes: { total: number; by_form: Array<[string, number]> };
  logs: { by_level: Array<[string, number]> };
  budget?: { errors_per_hour: number; warnings_per_hour: number; status: string };
  fleet: {
    workers_peak?: number | null;
    tested_delta?: number | null;
    found_delta?: number | null;
  };
  coordinator: {
    avg_cpu_usage_percent?: number | null;
  };
}

interface TopWorkerRow {
  worker_id: string;
  hostname: string;
  search_type: string;
  rate: number;
  tested: number;
  found: number;
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
  worker_id?: string,
  label_key?: string,
  label_value?: string
) {
  const params = new URLSearchParams({
    metrics: metrics.join(","),
    scope,
    from,
    to,
  });
  if (worker_id) params.set("worker_id", worker_id);
  if (label_key) params.set("label_key", label_key);
  if (label_value) params.set("label_value", label_value);
  const res = await fetch(`${API_BASE}/api/observability/metrics?${params}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const data = (await res.json()) as { series: MetricSeriesResponse[] };
  return data.series;
}

async function fetchLogs(
  from: string,
  to: string,
  filters: { level?: string; component?: string; worker?: string }
) {
  const params = new URLSearchParams({ from, to, limit: "200" });
  if (filters.level && filters.level !== "all") params.set("level", filters.level);
  if (filters.component) params.set("component", filters.component);
  if (filters.worker) params.set("worker_id", filters.worker);
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

async function fetchTopWorkers(limit: number, windowMinutes: number) {
  const params = new URLSearchParams({
    limit: limit.toString(),
    window_minutes: windowMinutes.toString(),
  });
  const res = await fetch(`${API_BASE}/api/observability/workers/top?${params}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const data = (await res.json()) as { workers: TopWorkerRow[] };
  return data.workers ?? [];
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
  const { fleet, coordinator, connected, searchJobs } = useWs();
  const [range, setRange] = useState(ranges[0]);
  const [series, setSeries] = useState<MetricSeriesResponse[]>([]);
  const [jobSeries, setJobSeries] = useState<MetricSeriesResponse[]>([]);
  const [selectedJobId, setSelectedJobId] = useState<string>("");
  const [logs, setLogs] = useState<LogsResponse["logs"]>([]);
  const [logLevelFilter, setLogLevelFilter] = useState("all");
  const [logComponentFilter, setLogComponentFilter] = useState("");
  const [logWorkerFilter, setLogWorkerFilter] = useState("");
  const [report, setReport] = useState<ReportResponse | null>(null);
  const [topWorkers, setTopWorkers] = useState<TopWorkerRow[]>([]);
  const [selectedTopWorkerId, setSelectedTopWorkerId] = useState<string>("");
  const [topWorkerSeries, setTopWorkerSeries] = useState<MetricPoint[]>([]);
  const [topWorkerWindow, setTopWorkerWindow] = useState(topWorkerWindows[1]);
  const [selectedMetricKeys, setSelectedMetricKeys] = useState(
    () => new Set(metricsCatalog.map((m) => m.key))
  );
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (selectedJobId) return;
    const active = searchJobs.find((j) => j.status === "running" || j.status === "paused");
    if (active) {
      setSelectedJobId(active.id.toString());
      return;
    }
    if (searchJobs.length > 0) {
      setSelectedJobId(searchJobs[0].id.toString());
    }
  }, [searchJobs, selectedJobId]);

  useEffect(() => {
    if (topWorkers.length === 0) {
      setSelectedTopWorkerId("");
      return;
    }
    const exists = topWorkers.some((w) => w.worker_id === selectedTopWorkerId);
    if (!exists) {
      setSelectedTopWorkerId(topWorkers[0].worker_id);
    }
  }, [topWorkers, selectedTopWorkerId]);

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
          "fleet.work_blocks_completed",
          "fleet.work_blocks_failed",
          "fleet.search_jobs_active",
          "fleet.max_heartbeat_age_secs",
        ],
        "fleet",
        from,
        to
      );
      const coordMetrics = await fetchMetrics(
        [
          "coordinator.cpu_usage_percent",
          "coordinator.memory_usage_percent",
          "coordinator.tick_drift_ms",
        ],
        "coordinator",
        from,
        to
      );
      const eventMetrics = await fetchMetrics(
        [
          "events.total_count",
          "events.error_count",
          "events.warning_count",
          "events.search_start_count",
          "events.search_done_count",
        ],
        "events",
        from,
        to
      );
      setSeries([...metrics, ...coordMetrics, ...eventMetrics]);

      const logsResp = await fetchLogs(from, to, {
        level: logLevelFilter,
        component: logComponentFilter.trim(),
        worker: logWorkerFilter.trim(),
      });
      setLogs(logsResp.logs ?? []);

      const reportResp = await fetchReport(from, to);
      setReport(reportResp);

      if (selectedJobId) {
        const jobMetrics = await fetchMetrics(
          ["search_job.completion_pct", "search_job.total_tested"],
          "search_job",
          from,
          to,
          undefined,
          "job_id",
          selectedJobId
        );
        setJobSeries(jobMetrics);
      } else {
        setJobSeries([]);
      }

      const top = await fetchTopWorkers(10, topWorkerWindow.minutes);
      setTopWorkers(top);

      if (selectedTopWorkerId) {
        const workerMetrics = await fetchMetrics(
          ["worker.tested"],
          "worker",
          from,
          to,
          selectedTopWorkerId
        );
        const seriesPoints = workerMetrics.find((m) => m.metric === "worker.tested");
        setTopWorkerSeries(seriesPoints?.points ?? []);
      } else {
        setTopWorkerSeries([]);
      }
    } catch {
      setSeries([]);
      setLogs([]);
      setReport(null);
      setJobSeries([]);
      setTopWorkers([]);
      setTopWorkerSeries([]);
    } finally {
      setLoading(false);
    }
  }, [
    range,
    selectedJobId,
    selectedTopWorkerId,
    topWorkerWindow,
    logLevelFilter,
    logComponentFilter,
    logWorkerFilter,
  ]);

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
    const completed = series.find((m) => m.metric === "fleet.work_blocks_completed");
    const failed = series.find((m) => m.metric === "fleet.work_blocks_failed");
    const merged: Array<{
      ts: string;
      available: number;
      claimed: number;
      completed: number;
      failed: number;
    }> = [];
    if (!available && !claimed && !completed && !failed) return merged;
    const map = new Map<
      string,
      { available?: number; claimed?: number; completed?: number; failed?: number }
    >();
    for (const p of available?.points ?? []) {
      map.set(p.ts, { available: p.value });
    }
    for (const p of claimed?.points ?? []) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, claimed: p.value });
    }
    for (const p of completed?.points ?? []) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, completed: p.value });
    }
    for (const p of failed?.points ?? []) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, failed: p.value });
    }
    for (const [ts, vals] of map.entries()) {
      merged.push({
        ts,
        available: vals.available ?? 0,
        claimed: vals.claimed ?? 0,
        completed: vals.completed ?? 0,
        failed: vals.failed ?? 0,
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

  const tickDriftSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "coordinator.tick_drift_ms");
    return s ? s.points : [];
  }, [series]);

  const heartbeatSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "fleet.max_heartbeat_age_secs");
    return s ? s.points : [];
  }, [series]);

  const errorEventsSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "events.error_count");
    return s ? s.points : [];
  }, [series]);

  const warningEventsSeries = useMemo(() => {
    const s = series.find((m) => m.metric === "events.warning_count");
    return s ? s.points : [];
  }, [series]);

  const stabilitySeries = useMemo(() => {
    const map = new Map<string, { error?: number; warning?: number }>();
    for (const p of errorEventsSeries) {
      map.set(p.ts, { error: p.value });
    }
    for (const p of warningEventsSeries) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, warning: p.value });
    }
    const merged: Array<{ ts: string; error: number; warning: number }> = [];
    for (const [ts, vals] of map.entries()) {
      merged.push({
        ts,
        error: vals.error ?? 0,
        warning: vals.warning ?? 0,
      });
    }
    merged.sort((a, b) => new Date(a.ts).getTime() - new Date(b.ts).getTime());
    return merged;
  }, [errorEventsSeries, warningEventsSeries]);

  const searchLifecycleSeries = useMemo(() => {
    const starts = series.find((m) => m.metric === "events.search_start_count");
    const dones = series.find((m) => m.metric === "events.search_done_count");
    const map = new Map<string, { start?: number; done?: number }>();
    for (const p of starts?.points ?? []) {
      map.set(p.ts, { start: p.value });
    }
    for (const p of dones?.points ?? []) {
      const prev = map.get(p.ts) ?? {};
      map.set(p.ts, { ...prev, done: p.value });
    }
    const merged: Array<{ ts: string; start: number; done: number }> = [];
    for (const [ts, vals] of map.entries()) {
      merged.push({
        ts,
        start: vals.start ?? 0,
        done: vals.done ?? 0,
      });
    }
    merged.sort((a, b) => new Date(a.ts).getTime() - new Date(b.ts).getTime());
    return merged;
  }, [series]);

  const latestErrorCount =
    errorEventsSeries.length > 0
      ? errorEventsSeries[errorEventsSeries.length - 1].value
      : 0;
  const latestWarningCount =
    warningEventsSeries.length > 0
      ? warningEventsSeries[warningEventsSeries.length - 1].value
      : 0;
  const errorPerHour = report?.budget?.errors_per_hour ?? latestErrorCount * 60;
  const warningPerHour = report?.budget?.warnings_per_hour ?? latestWarningCount * 60;
  const errorBudgetStatus =
    report?.budget?.status
      ? report.budget.status === "breached"
        ? "Breached"
        : report.budget.status === "risk"
          ? "Risk"
          : "Healthy"
      : errorPerHour > ERROR_BUDGET_ERRORS_PER_HOUR
        ? "Breached"
        : warningPerHour > ERROR_BUDGET_WARNINGS_PER_HOUR
          ? "Risk"
          : "Healthy";

  const jobCompletionSeries = useMemo(() => {
    const s = jobSeries.find((m) => m.metric === "search_job.completion_pct");
    return s ? s.points : [];
  }, [jobSeries]);

  const jobTestedSeries = useMemo(() => {
    const s = jobSeries.find((m) => m.metric === "search_job.total_tested");
    return s ? s.points : [];
  }, [jobSeries]);

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

  const onDownloadReportCsv = useCallback(() => {
    const { from, to } = toIsoRange(range.hours);
    const params = new URLSearchParams({ from, to, format: "csv" });
    const url = `${API_BASE}/api/observability/report?${params}`;
    const a = document.createElement("a");
    a.href = url;
    a.download = `observability-report-${new Date().toISOString()}.csv`;
    a.click();
  }, [range]);

  const onDownloadLogsCsv = useCallback(() => {
    const { from, to } = toIsoRange(range.hours);
    const params = new URLSearchParams({ from, to, format: "csv", limit: "2000" });
    if (logLevelFilter && logLevelFilter !== "all") params.set("level", logLevelFilter);
    if (logComponentFilter.trim()) params.set("component", logComponentFilter.trim());
    if (logWorkerFilter.trim()) params.set("worker_id", logWorkerFilter.trim());
    const url = `${API_BASE}/api/observability/logs?${params}`;
    const a = document.createElement("a");
    a.href = url;
    a.download = `observability-logs-${new Date().toISOString()}.csv`;
    a.click();
  }, [range, logLevelFilter, logComponentFilter, logWorkerFilter]);

  const onDownloadMetricsCsv = useCallback(() => {
    const { from, to } = toIsoRange(range.hours);
    const metrics = metricsCatalog
      .filter((m) => selectedMetricKeys.has(m.key))
      .map((m) => m.key);
    const params = new URLSearchParams({
      metrics: metrics.join(","),
      from,
      to,
      format: "csv",
    });
    const url = `${API_BASE}/api/observability/metrics?${params}`;
    const a = document.createElement("a");
    a.href = url;
    a.download = `observability-metrics-${new Date().toISOString()}.csv`;
    a.click();
  }, [range, selectedMetricKeys]);

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
            <Button size="sm" variant="outline" onClick={onDownloadReportCsv}>
              Export CSV
            </Button>
            <Button size="sm" variant="outline" onClick={onDownloadLogsCsv}>
              Export Logs CSV
            </Button>
            <Button size="sm" variant="outline" onClick={onDownloadMetricsCsv}>
              Export Metrics CSV
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
          label={`Error budget: ${errorBudgetStatus}`}
          value={`${numberWithCommas(Math.round(errorPerHour))}/hr`}
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
                <Line type="monotone" dataKey="completed" stroke="#60a5fa" dot={false} />
                <Line type="monotone" dataKey="failed" stroke="#f87171" dot={false} />
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
              Metrics export selection
            </CardTitle>
          </CardHeader>
          <CardContent className="grid grid-cols-1 md:grid-cols-2 gap-2">
            {metricsCatalog.map((metric) => {
              const checked = selectedMetricKeys.has(metric.key);
              return (
                <label
                  key={metric.key}
                  className="flex items-center gap-2 text-xs text-muted-foreground"
                >
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={(e) => {
                      setSelectedMetricKeys((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) {
                          next.add(metric.key);
                        } else {
                          next.delete(metric.key);
                        }
                        return next;
                      });
                    }}
                  />
                  <span className="text-foreground">{metric.label}</span>
                </label>
              );
            })}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Log filters
            </CardTitle>
          </CardHeader>
          <CardContent className="grid grid-cols-1 md:grid-cols-3 gap-2">
            <Select value={logLevelFilter} onValueChange={setLogLevelFilter}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder="Level" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All levels</SelectItem>
                <SelectItem value="error">Error</SelectItem>
                <SelectItem value="warn">Warn</SelectItem>
                <SelectItem value="info">Info</SelectItem>
                <SelectItem value="debug">Debug</SelectItem>
              </SelectContent>
            </Select>
            <Input
              value={logComponentFilter}
              onChange={(e) => setLogComponentFilter(e.target.value)}
              placeholder="Component"
              className="h-8 text-xs"
            />
            <Input
              value={logWorkerFilter}
              onChange={(e) => setLogWorkerFilter(e.target.value)}
              placeholder="Worker ID"
              className="h-8 text-xs"
            />
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Search lifecycle (per sample)
            </CardTitle>
          </CardHeader>
          <CardContent className="h-44">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={searchLifecycleSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis allowDecimals={false} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="start" stroke="#22c55e" dot={false} />
                <Line type="monotone" dataKey="done" stroke="#a855f7" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Error budget (per hour)
            </CardTitle>
          </CardHeader>
          <CardContent className="h-44 flex flex-col justify-center gap-2">
            <div className="flex items-center justify-between text-sm">
              <span>Errors/hr</span>
              <span className="tabular-nums font-semibold">
                {numberWithCommas(Math.round(errorPerHour))}
              </span>
            </div>
            <div className="flex items-center justify-between text-sm">
              <span>Warnings/hr</span>
              <span className="tabular-nums font-semibold">
                {numberWithCommas(Math.round(warningPerHour))}
              </span>
            </div>
            <div className="text-xs text-muted-foreground">
              Status: {errorBudgetStatus}
            </div>
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

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Max heartbeat age (minutes)
            </CardTitle>
          </CardHeader>
          <CardContent className="h-48">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart
                data={heartbeatSeries.map((p) => ({
                  ts: p.ts,
                  value: p.value / 60,
                }))}
              >
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="value" stroke="#f59e0b" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Errors & warnings per sample
            </CardTitle>
          </CardHeader>
          <CardContent className="h-48">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={stabilitySeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis allowDecimals={false} />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="error" stroke="#ef4444" dot={false} />
                <Line type="monotone" dataKey="warning" stroke="#f59e0b" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground">
              Coordinator tick drift (ms)
            </CardTitle>
          </CardHeader>
          <CardContent className="h-48">
            <ResponsiveContainer width="100%" height="100%">
              <LineChart data={tickDriftSeries}>
                <XAxis dataKey="ts" tickFormatter={(v) => new Date(v).toLocaleTimeString()} />
                <YAxis />
                <Tooltip labelFormatter={(v) => formatTime(v)} />
                <Line type="monotone" dataKey="value" stroke="#94a3b8" dot={false} />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
        <Card className="lg:col-span-2">
          <CardHeader className="pb-2 flex flex-col gap-2">
            <div className="flex items-center justify-between gap-3">
              <CardTitle className="text-xs font-medium text-muted-foreground">
                Search job efficiency
              </CardTitle>
              <Select
                value={selectedJobId}
                onValueChange={(v) => setSelectedJobId(v)}
              >
                <SelectTrigger className="h-8 w-[220px] text-xs">
                  <SelectValue placeholder="Select job" />
                </SelectTrigger>
                <SelectContent>
                  {searchJobs.length === 0 && (
                    <SelectItem value="none" disabled>
                      No jobs available
                    </SelectItem>
                  )}
                  {searchJobs.map((job) => (
                    <SelectItem key={job.id} value={job.id.toString()}>
                      #{job.id} · {job.search_type} · {job.status}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardHeader>
          <CardContent className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <div className="h-44">
              <div className="mb-2 text-xs text-muted-foreground">
                Completion %
              </div>
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={jobCompletionSeries}>
                  <XAxis
                    dataKey="ts"
                    tickFormatter={(v) => new Date(v).toLocaleTimeString()}
                  />
                  <YAxis domain={[0, 100]} />
                  <Tooltip labelFormatter={(v) => formatTime(v)} />
                  <Line type="monotone" dataKey="value" stroke="#34d399" dot={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>
            <div className="h-44">
              <div className="mb-2 text-xs text-muted-foreground">
                Tested rate (c/s)
              </div>
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={buildRateSeries(jobTestedSeries)}>
                  <XAxis
                    dataKey="ts"
                    tickFormatter={(v) => new Date(v).toLocaleTimeString()}
                  />
                  <YAxis tickFormatter={(v) => numberWithCommas(Math.round(v))} />
                  <Tooltip
                    formatter={(v: number | string | undefined) => [
                      numberWithCommas(Math.round(Number(v ?? 0))),
                      "c/s",
                    ]}
                    labelFormatter={(v) => formatTime(v)}
                  />
                  <Line type="monotone" dataKey="rate" stroke="#60a5fa" dot={false} />
                </LineChart>
              </ResponsiveContainer>
            </div>
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
        <div className="flex flex-col gap-4">
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
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-xs font-medium text-muted-foreground">
                Top workers ({topWorkerWindow.label} rate)
              </CardTitle>
            </CardHeader>
            <CardContent className="max-h-[260px] overflow-y-auto p-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Worker</TableHead>
                    <TableHead className="text-right">c/s</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {topWorkers.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={2} className="text-xs text-muted-foreground">
                        No worker rates available yet.
                      </TableCell>
                    </TableRow>
                  )}
                  {topWorkers.map((w) => (
                    <TableRow key={w.worker_id}>
                      <TableCell>
                        <div className="text-xs font-medium">
                          {w.hostname || w.worker_id}
                        </div>
                        <div className="text-[11px] text-muted-foreground">
                          {w.search_type || "unknown"} · {w.worker_id.slice(0, 8)}
                        </div>
                      </TableCell>
                      <TableCell className="text-right text-xs tabular-nums">
                        {numberWithCommas(Math.max(0, Math.round(w.rate)))}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
          <Card>
            <CardHeader className="pb-2 flex flex-col gap-2">
              <div className="flex items-center justify-between gap-3">
                <CardTitle className="text-xs font-medium text-muted-foreground">
                  Top worker throughput
                </CardTitle>
                <Select
                  value={topWorkerWindow.label}
                  onValueChange={(v) => {
                    const next = topWorkerWindows.find((w) => w.label === v);
                    if (next) setTopWorkerWindow(next);
                  }}
                >
                  <SelectTrigger className="h-8 w-[120px] text-xs">
                    <SelectValue placeholder="Window" />
                  </SelectTrigger>
                  <SelectContent>
                    {topWorkerWindows.map((w) => (
                      <SelectItem key={w.label} value={w.label}>
                        {w.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Select
                  value={selectedTopWorkerId}
                  onValueChange={(v) => setSelectedTopWorkerId(v)}
                >
                  <SelectTrigger className="h-8 w-[200px] text-xs">
                    <SelectValue placeholder="Select worker" />
                  </SelectTrigger>
                  <SelectContent>
                    {topWorkers.length === 0 && (
                      <SelectItem value="none" disabled>
                        No workers available
                      </SelectItem>
                    )}
                    {topWorkers.map((w) => (
                      <SelectItem key={w.worker_id} value={w.worker_id}>
                        {w.hostname || w.worker_id.slice(0, 8)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </CardHeader>
            <CardContent className="h-44">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={buildRateSeries(topWorkerSeries)}>
                  <XAxis
                    dataKey="ts"
                    tickFormatter={(v) => new Date(v).toLocaleTimeString()}
                  />
                  <YAxis tickFormatter={(v) => numberWithCommas(Math.round(v))} />
                  <Tooltip
                    formatter={(v: number | string | undefined) => [
                      numberWithCommas(Math.round(Number(v ?? 0))),
                      "c/s",
                    ]}
                    labelFormatter={(v) => formatTime(v)}
                  />
                  <Line type="monotone" dataKey="rate" stroke="#38bdf8" dot={false} />
                </LineChart>
              </ResponsiveContainer>
            </CardContent>
          </Card>
        </div>
      </div>
    </>
  );
}
