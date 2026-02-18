"use client";

/**
 * @module performance/page
 *
 * Performance monitoring page with real-time charts for fleet throughput,
 * per-worker rates, and algorithm-level metrics. Shows rolling time-series
 * data from WebSocket heartbeats — not persisted to Supabase.
 *
 * Charts: throughput over time (line), per-worker comparison (bar),
 * sieve efficiency ratios, and hardware utilization trends.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import {
  LineChart,
  Line,
  ResponsiveContainer,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
} from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { MetricsBar } from "@/components/metrics-bar";
import { useWs } from "@/contexts/websocket-context";
import { numberWithCommas, formatUptime } from "@/lib/format";
import type { WorkerStatus } from "@/hooks/use-websocket";
import { Activity, Cpu, HardDrive, MemoryStick, Gauge } from "lucide-react";
import { ViewHeader } from "@/components/view-header";

const MAX_HISTORY = 60;

interface ThroughputSample {
  time: number;
  rate: number;
}

interface WorkerRate {
  worker_id: string;
  hostname: string;
  search_type: string;
  rate: number;
  tested: number;
  found: number;
  uptime_secs: number;
  cores: number;
  cpu: number;
}

function estimateEta(
  tested: number,
  uptimeSecs: number,
  totalRange: number | null
): string | null {
  if (!totalRange || totalRange <= 0 || tested <= 0 || uptimeSecs <= 0)
    return null;
  const rate = tested / uptimeSecs;
  const remaining = totalRange - tested;
  if (remaining <= 0) return "Complete";
  const etaSecs = remaining / rate;
  if (etaSecs < 60) return `~${Math.ceil(etaSecs)}s`;
  if (etaSecs < 3600) return `~${Math.ceil(etaSecs / 60)}m`;
  if (etaSecs < 86400) {
    const h = Math.floor(etaSecs / 3600);
    const m = Math.ceil((etaSecs % 3600) / 60);
    return `~${h}h ${m}m`;
  }
  const d = Math.floor(etaSecs / 86400);
  const h = Math.ceil((etaSecs % 86400) / 3600);
  return `~${d}d ${h}h`;
}

function estimateTotalRange(worker: WorkerStatus): number | null {
  // Try to parse search_params JSON for range info
  try {
    const params = JSON.parse(worker.search_params);
    if (params.min_n != null && params.max_n != null) {
      // kbn, twin, sophie_germain, cullen_woodall, carol_kynea forms
      // Both +1 and -1 tested, so range * 2
      return (params.max_n - params.min_n + 1) * 2;
    }
    if (params.start != null && params.end != null) {
      // factorial, primorial — each n tests +1 and -1
      return (params.end - params.start + 1) * 2;
    }
    if (params.min_exp != null && params.max_exp != null) {
      // wagstaff
      return params.max_exp - params.min_exp + 1;
    }
    if (params.min_digits != null && params.max_digits != null) {
      // palindromic, near_repdigit — hard to estimate, return null
      return null;
    }
  } catch {
    // not JSON, ignore
  }
  return null;
}

export default function PerformancePage() {
  const { fleet, coordinator, searches } = useWs();
  const workers = useMemo(() => fleet?.workers ?? [], [fleet]);
  const prevRef = useRef<{ tested: number; time: number } | null>(null);
  const [currentRate, setCurrentRate] = useState(0);
  const [peakRate, setPeakRate] = useState(0);
  const [history, setHistory] = useState<ThroughputSample[]>([]);

  // Track per-worker rates
  const workerPrevRef = useRef<Map<string, { tested: number; time: number }>>(
    new Map()
  );
  const [workerRates, setWorkerRates] = useState<WorkerRate[]>([]);

  // Update aggregate throughput
  useEffect(() => {
    if (!fleet) return;
    const now = Date.now();
    const prev = prevRef.current;

    if (prev && fleet.total_tested >= prev.tested) {
      const dtSec = (now - prev.time) / 1000;
      if (dtSec > 0.5) {
        const r = Math.round((fleet.total_tested - prev.tested) / dtSec);
        setCurrentRate(r);
        setPeakRate((p) => Math.max(p, r));
        setHistory((h) => {
          const next = [...h, { time: now, rate: r }];
          return next.length > MAX_HISTORY ? next.slice(-MAX_HISTORY) : next;
        });
      }
    }
    prevRef.current = { tested: fleet.total_tested, time: now };
  }, [fleet?.total_tested, fleet]);

  // Update per-worker rates
  useEffect(() => {
    const now = Date.now();
    const prevMap = workerPrevRef.current;
    const rates: WorkerRate[] = [];

    for (const w of workers) {
      const prev = prevMap.get(w.worker_id);
      let rate = 0;
      if (prev && w.tested >= prev.tested) {
        const dtSec = (now - prev.time) / 1000;
        if (dtSec > 0.5) {
          rate = Math.round((w.tested - prev.tested) / dtSec);
        }
      }
      prevMap.set(w.worker_id, { tested: w.tested, time: now });
      rates.push({
        worker_id: w.worker_id,
        hostname: w.hostname,
        search_type: w.search_type,
        rate,
        tested: w.tested,
        found: w.found,
        uptime_secs: w.uptime_secs,
        cores: w.cores,
        cpu: w.metrics?.cpu_usage_percent ?? 0,
      });
    }

    rates.sort((a, b) => b.rate - a.rate);
    setWorkerRates(rates);
  }, [workers]);

  // Compute search ETAs
  const searchEtas = useMemo(() => {
    return workers
      .filter((w) => w.tested > 0 && w.uptime_secs > 0)
      .map((w) => {
        const totalRange = estimateTotalRange(w);
        const eta = estimateEta(w.tested, w.uptime_secs, totalRange);
        const pctDone = totalRange
          ? Math.min(100, (w.tested / totalRange) * 100)
          : null;
        return {
          worker_id: w.worker_id,
          search_type: w.search_type,
          current: w.current,
          tested: w.tested,
          totalRange,
          eta,
          pctDone,
        };
      })
      .filter((e) => e.eta !== null || e.pctDone !== null);
  }, [workers]);

  const totalTested = fleet?.total_tested ?? 0;
  const totalFound = fleet?.total_found ?? 0;
  const totalWorkers = fleet?.total_workers ?? 0;
  const totalCores = fleet?.total_cores ?? 0;

  // Build bar chart data from worker rates
  const workerBarData = useMemo(
    () =>
      workerRates
        .filter((w) => w.rate > 0)
        .map((w) => ({
          name: w.hostname || w.worker_id.slice(0, 8),
          rate: w.rate,
        })),
    [workerRates]
  );

  return (
    <>
      <ViewHeader
        title="Performance"
        subtitle={`${totalWorkers} workers · ${totalCores} cores · ${numberWithCommas(totalTested)} tested · ${totalFound} found`}
      />

      {/* Throughput hero */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
        <Card className="lg:col-span-2">
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
              <Gauge className="size-3.5" />
              Aggregate throughput
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="flex items-end gap-6 mb-3">
              <div>
                <div className="text-4xl font-semibold text-foreground tabular-nums">
                  {numberWithCommas(currentRate)}
                </div>
                <div className="text-sm text-muted-foreground">
                  candidates/sec
                </div>
              </div>
              <div className="text-right space-y-0.5">
                <div className="text-xs text-muted-foreground">
                  peak{" "}
                  <span className="tabular-nums font-medium text-foreground">
                    {numberWithCommas(peakRate)}
                  </span>
                </div>
                <div className="text-xs text-muted-foreground">
                  per-core{" "}
                  <span className="tabular-nums font-medium text-foreground">
                    {totalCores > 0
                      ? numberWithCommas(Math.round(currentRate / totalCores))
                      : "—"}
                  </span>
                </div>
              </div>
            </div>
            {history.length > 1 && (
              <div className="h-[100px]">
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={history}>
                    <Line
                      type="monotone"
                      dataKey="rate"
                      stroke="var(--chart-1)"
                      strokeWidth={1.5}
                      dot={false}
                      isAnimationActive={false}
                    />
                  </LineChart>
                </ResponsiveContainer>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Resource utilization */}
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
              <Activity className="size-3.5" />
              Coordinator resources
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            {coordinator ? (
              <>
                <MetricsBar
                  label="CPU"
                  percent={coordinator.cpu_usage_percent}
                />
                <MetricsBar
                  label="Memory"
                  percent={coordinator.memory_usage_percent}
                  detail={`${coordinator.memory_used_gb.toFixed(1)} / ${coordinator.memory_total_gb.toFixed(1)} GB`}
                />
                <MetricsBar
                  label="Disk"
                  percent={coordinator.disk_usage_percent}
                  detail={`${coordinator.disk_used_gb.toFixed(1)} / ${coordinator.disk_total_gb.toFixed(1)} GB`}
                />
                <div className="pt-1 text-xs text-muted-foreground space-y-0.5">
                  <div className="flex justify-between">
                    <span>Load (1m / 5m / 15m)</span>
                  </div>
                  <div className="font-mono tabular-nums text-foreground">
                    {coordinator.load_avg_1m.toFixed(2)} /{" "}
                    {coordinator.load_avg_5m.toFixed(2)} /{" "}
                    {coordinator.load_avg_15m.toFixed(2)}
                  </div>
                </div>
              </>
            ) : (
              <p className="text-xs text-muted-foreground">
                No coordinator metrics available
              </p>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Per-worker throughput */}
      {workerRates.length > 0 && (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <Cpu className="size-3.5" />
                Per-worker throughput
              </CardTitle>
            </CardHeader>
            <CardContent>
              {workerBarData.length > 0 ? (
                <div className="h-[200px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={workerBarData} layout="vertical">
                      <XAxis type="number" hide />
                      <YAxis
                        dataKey="name"
                        type="category"
                        width={80}
                        tick={{ fontSize: 11, fill: "var(--muted-foreground)" }}
                      />
                      <Tooltip
                        contentStyle={{
                          background: "var(--card)",
                          border: "1px solid var(--border)",
                          borderRadius: 8,
                          fontSize: 12,
                        }}
                        formatter={(value) => [
                          `${numberWithCommas(Number(value))} cand/s`,
                          "Rate",
                        ]}
                      />
                      <Bar
                        dataKey="rate"
                        fill="var(--chart-1)"
                        radius={[0, 4, 4, 0]}
                      />
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">
                  Waiting for throughput data...
                </p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader className="pb-2">
              <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
                <MemoryStick className="size-3.5" />
                Worker details
              </CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-2 max-h-[200px] overflow-y-auto">
                {workerRates.map((w) => (
                  <div
                    key={w.worker_id}
                    className="flex items-center justify-between text-xs"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="font-mono truncate max-w-[100px]">
                        {w.hostname || w.worker_id.slice(0, 8)}
                      </span>
                      <span className="text-muted-foreground truncate">
                        {w.search_type}
                      </span>
                    </div>
                    <div className="flex items-center gap-3 flex-shrink-0 tabular-nums">
                      <span>{numberWithCommas(w.rate)} c/s</span>
                      <span className="text-muted-foreground">
                        {w.cores}c &middot; {w.cpu.toFixed(0)}%
                      </span>
                      <span className="text-muted-foreground">
                        {formatUptime(w.uptime_secs)}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        </div>
      )}

      {/* Search progress & ETA */}
      {searchEtas.length > 0 && (
        <Card className="mb-6">
          <CardHeader className="pb-2">
            <CardTitle className="text-xs font-medium text-muted-foreground flex items-center gap-1.5">
              <HardDrive className="size-3.5" />
              Search progress & ETA
            </CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-3">
              {searchEtas.map((s) => (
                <div key={s.worker_id} className="space-y-1">
                  <div className="flex items-center justify-between text-xs">
                    <div className="flex items-center gap-2 min-w-0">
                      <span className="font-medium text-foreground">
                        {s.search_type}
                      </span>
                      <span className="text-muted-foreground truncate max-w-[300px]">
                        {s.current}
                      </span>
                    </div>
                    <div className="flex items-center gap-3 flex-shrink-0 tabular-nums text-muted-foreground">
                      {s.pctDone !== null && (
                        <span className="font-medium text-foreground">
                          {s.pctDone.toFixed(1)}%
                        </span>
                      )}
                      {s.eta && <span>ETA {s.eta}</span>}
                      <span>{numberWithCommas(s.tested)} tested</span>
                    </div>
                  </div>
                  {s.pctDone !== null && (
                    <div className="h-1.5 bg-muted rounded-full overflow-hidden">
                      <div
                        className="h-full bg-primary rounded-full transition-all duration-1000"
                        style={{ width: `${Math.min(100, s.pctDone)}%` }}
                      />
                    </div>
                  )}
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      )}

      {/* Empty state */}
      {totalWorkers === 0 && (
        <Card className="py-8 border-dashed">
          <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
            No workers connected. Start a search to see performance metrics.
          </CardContent>
        </Card>
      )}
    </>
  );
}
