"use client";

/**
 * @module fleet/page
 *
 * Fleet monitoring page. Shows all server nodes grouped by hostname,
 * each with hardware metrics (CPU/memory/disk), running workers,
 * active searches, and remote deployments. Provides controls to add
 * new servers, deploy searches, and stop workers.
 *
 * Data comes from the WebSocket (fleet heartbeats, not Supabase).
 */

import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useWs } from "@/contexts/websocket-context";
import { AddServerDialog } from "@/components/add-server-dialog";
import { NewSearchDialog } from "@/components/new-search-dialog";
import { SearchCard } from "@/components/search-card";
import { MetricsBar } from "@/components/metrics-bar";
import { WorkerDetailDialog } from "@/components/worker-detail-dialog";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { API_BASE, formatTime, numberWithCommas } from "@/lib/format";
import { ViewHeader } from "@/components/view-header";
import type { Deployment, WorkerStatus } from "@/hooks/use-websocket";

type WorkerHealth = "healthy" | "stale" | "offline";

function workerHealth(worker: WorkerStatus): WorkerHealth {
  if (worker.last_heartbeat_secs_ago < 30) return "healthy";
  if (worker.last_heartbeat_secs_ago < 60) return "stale";
  return "offline";
}

function parseJsonObject(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}

function formatWorkerParams(searchType: string, params: Record<string, unknown>): string {
  switch (searchType) {
    case "kbn":
      return `k=${params.k ?? "?"}, base=${params.base ?? "?"}, n=[${params.min_n ?? "?"},${params.max_n ?? "?"}]`;
    case "factorial":
      return `n=[${params.start ?? "?"},${params.end ?? "?"}]`;
    case "palindromic":
      return `base=${params.base ?? "?"}, digits=[${params.min_digits ?? "?"},${params.max_digits ?? "?"}]`;
    default:
      return JSON.stringify(params);
  }
}

function deploymentStatusDot(status: string): string {
  if (status === "running") return "bg-green-500";
  if (status === "deploying") return "bg-yellow-500 animate-pulse";
  if (status === "paused") return "bg-yellow-500";
  if (status === "failed") return "bg-red-500";
  return "bg-muted-foreground";
}

function healthPillClass(health: WorkerHealth): string {
  if (health === "healthy") return "border-emerald-500/40 bg-emerald-500/10 text-emerald-400";
  if (health === "stale") return "border-amber-500/40 bg-amber-500/10 text-amber-300";
  return "border-red-500/40 bg-red-500/10 text-red-300";
}

function deploymentPillClass(status: string): string {
  if (status === "running") return "border-emerald-500/40 bg-emerald-500/10 text-emerald-400";
  if (status === "deploying") return "border-amber-500/40 bg-amber-500/10 text-amber-300";
  if (status === "paused") return "border-amber-500/40 bg-amber-500/10 text-amber-300";
  if (status === "failed") return "border-red-500/40 bg-red-500/10 text-red-300";
  return "border-border bg-muted/30 text-muted-foreground";
}

export default function FleetPage() {
  const { fleet, coordinator, deployments, searches } = useWs();
  const [addServerOpen, setAddServerOpen] = useState(false);
  const [newSearchOpen, setNewSearchOpen] = useState(false);
  const [selectedWorker, setSelectedWorker] = useState<WorkerStatus | null>(null);
  const [workerDetailOpen, setWorkerDetailOpen] = useState(false);
  const [stoppingDeploymentId, setStoppingDeploymentId] = useState<number | null>(null);
  const [pausingDeploymentId, setPausingDeploymentId] = useState<number | null>(null);
  const [resumingDeploymentId, setResumingDeploymentId] = useState<number | null>(null);
  const [stoppingWorkerId, setStoppingWorkerId] = useState<string | null>(null);
  const [searchFilter, setSearchFilter] = useState("");
  const [healthFilter, setHealthFilter] = useState<"all" | WorkerHealth>("all");
  const [typeFilter, setTypeFilter] = useState<"all" | "kbn" | "factorial" | "palindromic">("all");

  const workers = useMemo(() => fleet?.workers ?? [], [fleet]);

  const fleetRate = useMemo(() => {
    return workers.reduce((acc, w) => {
      if (w.uptime_secs <= 0) return acc;
      return acc + w.tested / w.uptime_secs;
    }, 0);
  }, [workers]);

  const healthCounts = useMemo(() => {
    return workers.reduce(
      (acc, w) => {
        const h = workerHealth(w);
        acc[h] += 1;
        return acc;
      },
      { healthy: 0, stale: 0, offline: 0 }
    );
  }, [workers]);

  const filteredWorkers = useMemo(() => {
    const query = searchFilter.trim().toLowerCase();
    return workers
      .filter((w) => {
        if (healthFilter !== "all" && workerHealth(w) !== healthFilter) return false;
        if (typeFilter !== "all" && w.search_type !== typeFilter) return false;
        if (!query) return true;
        return (
          w.worker_id.toLowerCase().includes(query) ||
          w.hostname.toLowerCase().includes(query) ||
          w.current.toLowerCase().includes(query)
        );
      })
      .sort((a, b) => {
        if (a.last_heartbeat_secs_ago !== b.last_heartbeat_secs_ago) {
          return a.last_heartbeat_secs_ago - b.last_heartbeat_secs_ago;
        }
        return b.tested - a.tested;
      });
  }, [workers, healthFilter, typeFilter, searchFilter]);

  const activeDeployments = useMemo(
    () => deployments.filter((d) => d.status === "running" || d.status === "deploying" || d.status === "paused"),
    [deployments]
  );

  async function stopDeployment(deployment: Deployment) {
    setStoppingDeploymentId(deployment.id);
    try {
      const res = await fetch(`${API_BASE}/api/fleet/deploy/${deployment.id}`, {
        method: "DELETE",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success(`Stopped deployment #${deployment.id}`);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to stop deployment";
      toast.error(message);
    } finally {
      setStoppingDeploymentId(null);
    }
  }

  async function pauseDeployment(deployment: Deployment) {
    setPausingDeploymentId(deployment.id);
    try {
      const res = await fetch(`${API_BASE}/api/fleet/deploy/${deployment.id}/pause`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success(`Paused deployment #${deployment.id}`);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to pause deployment";
      toast.error(message);
    } finally {
      setPausingDeploymentId(null);
    }
  }

  async function resumeDeployment(deployment: Deployment) {
    setResumingDeploymentId(deployment.id);
    try {
      const res = await fetch(`${API_BASE}/api/fleet/deploy/${deployment.id}/resume`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success(`Resuming deployment #${deployment.id}`);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to resume deployment";
      toast.error(message);
    } finally {
      setResumingDeploymentId(null);
    }
  }

  async function stopWorker(workerId: string) {
    setStoppingWorkerId(workerId);
    try {
      const res = await fetch(`${API_BASE}/api/fleet/workers/${encodeURIComponent(workerId)}/stop`, {
        method: "POST",
      });
      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        throw new Error(data.error || `HTTP ${res.status}`);
      }
      toast.success(`Stop command sent to ${workerId}`);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Failed to stop worker";
      toast.error(message);
    } finally {
      setStoppingWorkerId(null);
    }
  }

  return (
    <>
      <ViewHeader
        title="Fleet"
        subtitle="Worker health, deployment lifecycle, and distributed search operations."
        actions={
          <>
            <Button variant="outline" size="sm" onClick={() => setNewSearchOpen(true)}>
              New Search
            </Button>
            <Button size="sm" onClick={() => setAddServerOpen(true)}>Add Server</Button>
          </>
        }
        className="mb-5"
      />

      <div className="grid grid-cols-2 lg:grid-cols-5 gap-3 mb-5">
        <StatCard label="Workers" value={numberWithCommas(fleet?.total_workers ?? 0)} />
        <StatCard label="Cores" value={numberWithCommas(fleet?.total_cores ?? 0)} />
        <StatCard label="Fleet Throughput" value={`${numberWithCommas(Math.round(fleetRate))}/s`} />
        <StatCard label="Tested" value={numberWithCommas(fleet?.total_tested ?? 0)} />
        <StatCard label="Found" value={numberWithCommas(fleet?.total_found ?? 0)} />
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-5 mb-5">
        <Card className="xl:col-span-2 rounded-md shadow-none">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Workers</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
              <Input
                placeholder="Filter by worker, host, or candidate..."
                value={searchFilter}
                onChange={(e) => setSearchFilter(e.target.value)}
              />
              <Select
                value={healthFilter}
                onValueChange={(v) => setHealthFilter(v as "all" | WorkerHealth)}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Health" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All health</SelectItem>
                  <SelectItem value="healthy">Healthy</SelectItem>
                  <SelectItem value="stale">Stale</SelectItem>
                  <SelectItem value="offline">Offline</SelectItem>
                </SelectContent>
              </Select>
              <Select
                value={typeFilter}
                onValueChange={(v) =>
                  setTypeFilter(v as "all" | "kbn" | "factorial" | "palindromic")
                }
              >
                <SelectTrigger>
                  <SelectValue placeholder="Search type" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All search types</SelectItem>
                  <SelectItem value="kbn">KBN</SelectItem>
                  <SelectItem value="factorial">Factorial</SelectItem>
                  <SelectItem value="palindromic">Palindromic</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {filteredWorkers.length === 0 ? (
              <EmptyState message="No workers match current filters." />
            ) : (
              <div className="space-y-2">
                {filteredWorkers.map((worker) => {
                  const health = workerHealth(worker);
                  const throughput =
                    worker.uptime_secs > 0
                      ? (worker.tested / worker.uptime_secs).toFixed(1)
                      : "0.0";
                  const params = parseJsonObject(worker.search_params);
                  return (
                    <Card key={worker.worker_id} className="py-3 rounded-md shadow-none bg-card/40">
                      <CardContent className="p-0 px-4">
                        <div className="flex flex-wrap items-center gap-2 justify-between">
                          <div className="flex items-center gap-2 min-w-0">
                            <div
                              className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${
                                health === "healthy"
                                  ? "bg-green-500"
                                  : health === "stale"
                                    ? "bg-yellow-500"
                                    : "bg-red-500"
                              }`}
                            />
                            <span className="font-medium truncate">{worker.hostname}</span>
                            <span className="text-xs text-muted-foreground font-mono truncate">
                              {worker.worker_id}
                            </span>
                          </div>
                          <div className="flex items-center gap-2">
                            <Badge variant="outline" className="font-mono text-xs">{worker.search_type}</Badge>
                            <span className={`text-xs px-2 py-0.5 rounded-full border ${healthPillClass(health)}`}>
                              {health}
                            </span>
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-7"
                              onClick={() => {
                                setSelectedWorker(worker);
                                setWorkerDetailOpen(true);
                              }}
                            >
                              Inspect
                            </Button>
                            {health === "healthy" && (
                              <Button
                                size="sm"
                                variant="outline"
                                className="h-7 text-red-600 hover:text-red-700"
                                disabled={stoppingWorkerId === worker.worker_id}
                                onClick={() => void stopWorker(worker.worker_id)}
                              >
                                {stoppingWorkerId === worker.worker_id ? "Stopping..." : "Stop"}
                              </Button>
                            )}
                          </div>
                        </div>
                        <div className="mt-2 text-xs text-muted-foreground truncate font-mono">
                          {params ? formatWorkerParams(worker.search_type, params) : worker.search_params}
                        </div>
                        <div className="mt-2 grid grid-cols-2 md:grid-cols-5 gap-2 text-xs text-muted-foreground font-mono">
                          <span>{worker.cores} cores</span>
                          <span>{numberWithCommas(worker.tested)} tested</span>
                          <span>{worker.found} found</span>
                          <span>{throughput}/s</span>
                          <span>
                            hb{" "}
                            {worker.last_heartbeat_secs_ago < 5
                              ? "now"
                              : `${worker.last_heartbeat_secs_ago}s`}
                          </span>
                        </div>
                        {worker.metrics && (
                          <div className="mt-3 space-y-2">
                            <MetricsBar label="CPU" percent={worker.metrics.cpu_usage_percent} />
                            <MetricsBar
                              label="Memory"
                              percent={worker.metrics.memory_usage_percent}
                              detail={`${worker.metrics.memory_used_gb} / ${worker.metrics.memory_total_gb} GB`}
                            />
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  );
                })}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="rounded-md shadow-none">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Health Summary</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between text-sm">
              <span className="text-muted-foreground">Healthy</span>
              <Badge variant="default">{healthCounts.healthy}</Badge>
            </div>
            <div className="flex items-center justify-between text-sm">
              <span className="text-muted-foreground">Stale</span>
              <Badge variant="secondary">{healthCounts.stale}</Badge>
            </div>
            <div className="flex items-center justify-between text-sm">
              <span className="text-muted-foreground">Offline</span>
              <Badge variant="destructive">{healthCounts.offline}</Badge>
            </div>
            <div className="pt-3 border-t space-y-2">
              <div className="text-xs font-medium text-muted-foreground">
                Active Deployments
              </div>
              <div className="text-2xl font-semibold">{activeDeployments.length}</div>
            </div>
            <div className="pt-3 border-t space-y-2">
              <div className="text-xs font-medium text-muted-foreground">
                Running Searches
              </div>
              <div className="text-2xl font-semibold">
                {searches.filter((s) => s.status === "running").length}
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card className="mb-5 rounded-md shadow-none">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Deployments</CardTitle>
        </CardHeader>
        <CardContent>
          {deployments.length === 0 ? (
            <EmptyState message="No deployments yet." />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-xs font-medium text-muted-foreground">Status</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Host</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Type</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Params</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Worker</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Started</TableHead>
                  <TableHead className="text-right text-xs font-medium text-muted-foreground">Control</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {deployments.map((deployment) => (
                  <TableRow key={deployment.id} className="hover:bg-muted/30">
                    <TableCell>
                      <div className="flex items-center gap-2">
                        <div
                          className={`w-2 h-2 rounded-full ${deploymentStatusDot(deployment.status)}`}
                        />
                        <span className={`capitalize text-xs px-2 py-0.5 rounded-full border ${deploymentPillClass(deployment.status)}`}>
                          {deployment.status}
                        </span>
                      </div>
                    </TableCell>
                    <TableCell className="font-medium">
                      {deployment.ssh_user}@{deployment.hostname}
                    </TableCell>
                    <TableCell>
                      <Badge variant="outline" className="font-mono text-xs">{deployment.search_type}</Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground font-mono text-xs">
                      {deployment.search_params}
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {deployment.worker_id}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatTime(deployment.started_at)}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex items-center justify-end gap-1">
                        {deployment.status === "running" && (
                          <>
                            <Button
                              size="sm"
                              variant="outline"
                              disabled={pausingDeploymentId === deployment.id}
                              onClick={() => void pauseDeployment(deployment)}
                            >
                              {pausingDeploymentId === deployment.id ? "Pausing..." : "Pause"}
                            </Button>
                            <Button
                              size="sm"
                              variant="destructive"
                              disabled={stoppingDeploymentId === deployment.id}
                              onClick={() => void stopDeployment(deployment)}
                            >
                              {stoppingDeploymentId === deployment.id ? "Stopping..." : "Stop"}
                            </Button>
                          </>
                        )}
                        {deployment.status === "paused" && (
                          <Button
                            size="sm"
                            variant="default"
                            disabled={resumingDeploymentId === deployment.id}
                            onClick={() => void resumeDeployment(deployment)}
                          >
                            {resumingDeploymentId === deployment.id ? "Resuming..." : "Resume"}
                          </Button>
                        )}
                        {deployment.status !== "running" && deployment.status !== "paused" && (
                          <span className="text-xs text-muted-foreground">-</span>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Card className="rounded-md shadow-none">
        <CardHeader className="pb-2">
          <CardTitle className="text-base">Search Queue</CardTitle>
        </CardHeader>
        <CardContent>
          {searches.length === 0 ? (
            <EmptyState message="No searches in queue." />
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
              {[...searches]
                .sort((a, b) => {
                  const aRunning = a.status === "running" ? 0 : 1;
                  const bRunning = b.status === "running" ? 0 : 1;
                  if (aRunning !== bRunning) return aRunning - bRunning;
                  return new Date(b.started_at).getTime() - new Date(a.started_at).getTime();
                })
                .map((search) => (
                  <SearchCard key={search.id} search={search} />
                ))}
            </div>
          )}
        </CardContent>
      </Card>

      {coordinator && (
        <Card className="mt-5 rounded-md shadow-none">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Coordinator Metrics</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <MetricsBar label="CPU" percent={coordinator.cpu_usage_percent} />
            <MetricsBar
              label="Memory"
              percent={coordinator.memory_usage_percent}
              detail={`${coordinator.memory_used_gb} / ${coordinator.memory_total_gb} GB`}
            />
            <MetricsBar
              label="Disk"
              percent={coordinator.disk_usage_percent}
              detail={`${coordinator.disk_used_gb} / ${coordinator.disk_total_gb} GB`}
            />
            <div className="text-xs text-muted-foreground">
              Load: {coordinator.load_avg_1m} / {coordinator.load_avg_5m} / {coordinator.load_avg_15m}
            </div>
          </CardContent>
        </Card>
      )}

      <AddServerDialog
        open={addServerOpen}
        onOpenChange={setAddServerOpen}
        onDeployed={() => toast.success("Worker deployment started")}
      />

      <NewSearchDialog
        open={newSearchOpen}
        onOpenChange={setNewSearchOpen}
        onCreated={() => toast.success("Search started")}
      />

      <WorkerDetailDialog
        worker={selectedWorker}
        open={workerDetailOpen}
        onOpenChange={(open) => {
          if (!open) {
            setWorkerDetailOpen(false);
            setSelectedWorker(null);
          }
        }}
      />
    </>
  );
}
