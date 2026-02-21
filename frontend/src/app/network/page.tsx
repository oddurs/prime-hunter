"use client";

/**
 * @module fleet/page
 *
 * Fleet monitoring page. Shows servers grouped by role (service vs compute),
 * with hardware metrics, workers nested under their host servers, active
 * searches, and remote deployments.
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
import type { Deployment, ServerInfo, WorkerStatus } from "@/hooks/use-websocket";

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

function roleBadgeClass(role: string): string {
  if (role === "service") return "border-blue-500/40 bg-blue-500/10 text-blue-400";
  return "border-emerald-500/40 bg-emerald-500/10 text-emerald-400";
}

/** Derive server status from metrics or workers */
function serverStatus(server: ServerInfo, workers: WorkerStatus[]): "online" | "degraded" | "offline" {
  if (server.role === "service") {
    // Service server is online if we have metrics for it (we're connected to it)
    return server.metrics ? "online" : "degraded";
  }
  const hostWorkers = workers.filter((w) => server.worker_ids.includes(w.worker_id));
  if (hostWorkers.length === 0) return "offline";
  const allHealthy = hostWorkers.every((w) => workerHealth(w) === "healthy");
  if (allHealthy) return "online";
  const anyHealthy = hostWorkers.some((w) => workerHealth(w) === "healthy");
  return anyHealthy ? "degraded" : "offline";
}

function statusDotClass(status: "online" | "degraded" | "offline"): string {
  if (status === "online") return "bg-green-500";
  if (status === "degraded") return "bg-yellow-500";
  return "bg-red-500";
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

  // Compute servers from backend data, falling back to client-side grouping
  const servers = useMemo((): ServerInfo[] => {
    if (fleet?.servers && fleet.servers.length > 0) return fleet.servers;
    // Fallback: group workers by hostname (no coordinator info available)
    const hostMap = new Map<string, WorkerStatus[]>();
    for (const w of workers) {
      const list = hostMap.get(w.hostname) ?? [];
      list.push(w);
      hostMap.set(w.hostname, list);
    }
    return Array.from(hostMap.entries()).map(([hostname, hw]) => ({
      hostname,
      role: "compute" as const,
      metrics: hw[0]?.metrics ?? null,
      worker_count: hw.length,
      cores: hw.reduce((s, w) => s + w.cores, 0),
      worker_ids: hw.map((w) => w.worker_id),
      total_tested: hw.reduce((s, w) => s + w.tested, 0),
      total_found: hw.reduce((s, w) => s + w.found, 0),
      uptime_secs: Math.max(...hw.map((w) => w.uptime_secs), 0),
    }));
  }, [fleet, workers]);

  const serviceServers = useMemo(() => servers.filter((s) => s.role === "service"), [servers]);
  const computeServers = useMemo(() => servers.filter((s) => s.role === "compute"), [servers]);
  const totalComputeWorkers = useMemo(() => computeServers.reduce((s, c) => s + c.worker_count, 0), [computeServers]);

  // Filter workers for display inside compute server cards
  const filteredWorkerIds = useMemo(() => {
    const query = searchFilter.trim().toLowerCase();
    const set = new Set<string>();
    for (const w of workers) {
      if (healthFilter !== "all" && workerHealth(w) !== healthFilter) continue;
      if (typeFilter !== "all" && w.search_type !== typeFilter) continue;
      if (query && !(
        w.worker_id.toLowerCase().includes(query) ||
        w.hostname.toLowerCase().includes(query) ||
        w.current.toLowerCase().includes(query)
      )) continue;
      set.add(w.worker_id);
    }
    return set;
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

  function renderWorkerRow(worker: WorkerStatus) {
    const health = workerHealth(worker);
    const throughput =
      worker.uptime_secs > 0
        ? (worker.tested / worker.uptime_secs).toFixed(1)
        : "0.0";
    const params = parseJsonObject(worker.search_params);
    return (
      <div key={worker.worker_id} className="flex flex-wrap items-center gap-2 justify-between py-2 px-3 rounded bg-muted/20">
        <div className="flex items-center gap-2 min-w-0">
          <div
            className={`w-2 h-2 rounded-full flex-shrink-0 ${
              health === "healthy"
                ? "bg-green-500"
                : health === "stale"
                  ? "bg-yellow-500"
                  : "bg-red-500"
            }`}
          />
          <span className="font-mono text-xs truncate">{worker.worker_id}</span>
          <Badge variant="outline" className="font-mono text-xs">{worker.search_type}</Badge>
          <span className={`text-xs px-1.5 py-0.5 rounded-full border ${healthPillClass(health)}`}>
            {health}
          </span>
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground font-mono">
          <span>{params ? formatWorkerParams(worker.search_type, params) : worker.search_params}</span>
          <span>{numberWithCommas(worker.tested)} tested</span>
          <span>{throughput}/s</span>
          <Button
            size="sm"
            variant="outline"
            className="h-6 text-xs"
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
              className="h-6 text-xs text-red-600 hover:text-red-700"
              disabled={stoppingWorkerId === worker.worker_id}
              onClick={() => void stopWorker(worker.worker_id)}
            >
              {stoppingWorkerId === worker.worker_id ? "Stopping..." : "Stop"}
            </Button>
          )}
        </div>
      </div>
    );
  }

  return (
    <>
      <ViewHeader
        title="Fleet"
        subtitle="Server roles, worker health, deployment lifecycle, and distributed search operations."
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

      <div className="grid grid-cols-2 lg:grid-cols-6 gap-3 mb-5">
        <StatCard label="Servers" value={numberWithCommas(servers.length)} />
        <StatCard label="Workers" value={numberWithCommas(fleet?.total_workers ?? 0)} />
        <StatCard label="Cores" value={numberWithCommas(fleet?.total_cores ?? 0)} />
        <StatCard label="Fleet Throughput" value={`${numberWithCommas(Math.round(fleetRate))}/s`} />
        <StatCard label="Tested" value={numberWithCommas(fleet?.total_tested ?? 0)} />
        <StatCard label="Found" value={numberWithCommas(fleet?.total_found ?? 0)} />
      </div>

      {/* Filters */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2 mb-5">
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

      {/* Service Servers */}
      {serviceServers.length > 0 && (
        <div className="mb-5">
          <h2 className="text-sm font-medium text-muted-foreground mb-3">
            Service ({serviceServers.length} server{serviceServers.length !== 1 ? "s" : ""})
          </h2>
          <div className="space-y-3">
            {serviceServers.map((server) => {
              const status = serverStatus(server, workers);
              return (
                <Card key={server.hostname} className="rounded-md shadow-none">
                  <CardContent className="p-4">
                    <div className="flex items-center gap-2 mb-2">
                      <div className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${statusDotClass(status)}`} />
                      <span className="font-medium">{server.hostname}</span>
                      <Badge variant="outline" className={`text-xs uppercase ${roleBadgeClass(server.role)}`}>
                        Service
                      </Badge>
                    </div>
                    <div className="text-xs text-muted-foreground mb-3">
                      Coordinator, Dashboard, API, WebSocket
                    </div>
                    {server.metrics && (
                      <div className="space-y-2">
                        <MetricsBar label="CPU" percent={server.metrics.cpu_usage_percent} />
                        <MetricsBar
                          label="Memory"
                          percent={server.metrics.memory_usage_percent}
                          detail={`${server.metrics.memory_used_gb} / ${server.metrics.memory_total_gb} GB`}
                        />
                        <MetricsBar
                          label="Disk"
                          percent={server.metrics.disk_usage_percent}
                          detail={`${server.metrics.disk_used_gb} / ${server.metrics.disk_total_gb} GB`}
                        />
                        <div className="text-xs text-muted-foreground">
                          Load: {server.metrics.load_avg_1m} / {server.metrics.load_avg_5m} / {server.metrics.load_avg_15m}
                        </div>
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
          </div>
        </div>
      )}

      {/* Compute Servers */}
      <div className="mb-5">
        <h2 className="text-sm font-medium text-muted-foreground mb-3">
          Compute ({computeServers.length} server{computeServers.length !== 1 ? "s" : ""}, {totalComputeWorkers} worker{totalComputeWorkers !== 1 ? "s" : ""})
        </h2>
        {computeServers.length === 0 ? (
          <EmptyState message="No compute servers online." />
        ) : (
          <div className="space-y-3">
            {computeServers.map((server) => {
              const status = serverStatus(server, workers);
              const serverWorkers = workers
                .filter((w) => server.worker_ids.includes(w.worker_id) && filteredWorkerIds.has(w.worker_id))
                .sort((a, b) => a.worker_id.localeCompare(b.worker_id));
              const throughput = server.uptime_secs > 0
                ? (server.total_tested / server.uptime_secs).toFixed(1)
                : "0.0";
              return (
                <Card key={server.hostname} className="rounded-md shadow-none">
                  <CardContent className="p-4">
                    <div className="flex flex-wrap items-center gap-2 justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <div className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${statusDotClass(status)}`} />
                        <span className="font-medium">{server.hostname}</span>
                        <Badge variant="outline" className={`text-xs uppercase ${roleBadgeClass(server.role)}`}>
                          Compute
                        </Badge>
                      </div>
                      <div className="flex items-center gap-3 text-xs text-muted-foreground font-mono">
                        <span>{server.worker_count} worker{server.worker_count !== 1 ? "s" : ""}</span>
                        <span>{server.cores} core{server.cores !== 1 ? "s" : ""}</span>
                        <span>{numberWithCommas(server.total_tested)} tested</span>
                        <span>{throughput}/s</span>
                      </div>
                    </div>
                    {server.metrics && (
                      <div className="space-y-2 mb-3">
                        <MetricsBar label="CPU" percent={server.metrics.cpu_usage_percent} />
                        <MetricsBar
                          label="Memory"
                          percent={server.metrics.memory_usage_percent}
                          detail={`${server.metrics.memory_used_gb} / ${server.metrics.memory_total_gb} GB`}
                        />
                      </div>
                    )}
                    {serverWorkers.length > 0 && (
                      <div className="space-y-1.5 mt-2">
                        {serverWorkers.map(renderWorkerRow)}
                      </div>
                    )}
                    {serverWorkers.length === 0 && server.worker_count > 0 && (
                      <div className="text-xs text-muted-foreground mt-2">
                        No workers match current filters.
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
          </div>
        )}
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
