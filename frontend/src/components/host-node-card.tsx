"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { MetricsBar } from "@/components/metrics-bar";
import { ProcessRow } from "@/components/process-row";
import type { WorkerStatus, ManagedSearch, HardwareMetrics, Deployment } from "@/hooks/use-websocket";

export interface HostNode {
  hostname: string;
  isCoordinator: boolean;
  metrics: HardwareMetrics | null;
  workers: WorkerStatus[];
  searches: ManagedSearch[];
  deployments: Deployment[];
  totalCores: number;
  totalTested: number;
  totalFound: number;
}

function accentColor(node: HostNode): string {
  if (node.isCoordinator) return "border-l-blue-500";
  // Check worst worker health
  const worstHeartbeat = Math.max(...node.workers.map((w) => w.last_heartbeat_secs_ago), 0);
  if (worstHeartbeat > 60) return "border-l-red-500";
  if (worstHeartbeat > 30) return "border-l-yellow-500";
  return "border-l-green-500";
}

function statusDotColor(node: HostNode): string {
  if (node.workers.length === 0) return "bg-muted-foreground";
  const worstHeartbeat = Math.max(...node.workers.map((w) => w.last_heartbeat_secs_ago), 0);
  if (worstHeartbeat > 60) return "bg-red-500";
  if (worstHeartbeat > 30) return "bg-yellow-500";
  return "bg-green-500";
}

interface HostNodeCardProps {
  node: HostNode;
  onInspectWorker?: (worker: WorkerStatus) => void;
}

export function HostNodeCard({ node, onInspectWorker }: HostNodeCardProps) {
  const m = node.metrics;

  return (
    <Card className={`py-3 border-l-4 ${accentColor(node)}`}>
      <CardContent className="p-0 px-4 space-y-3">
        {/* Header: status dot + hostname + role badge + cores */}
        <div className="flex items-center gap-2 flex-wrap">
          <div className={`size-2.5 rounded-full flex-shrink-0 ${statusDotColor(node)}`} />
          <span className="text-sm font-semibold text-foreground">{node.hostname}</span>
          {node.isCoordinator && (
            <Badge variant="outline" className="text-[10px] px-1.5 py-0">Coordinator</Badge>
          )}
          <span className="text-xs text-muted-foreground ml-auto">
            {node.totalCores} core{node.totalCores !== 1 ? "s" : ""}
          </span>
        </div>

        {/* Hardware metrics — shown once per host */}
        {m && (
          <div className="space-y-2">
            <MetricsBar label="CPU" percent={m.cpu_usage_percent} />
            <MetricsBar
              label="Mem"
              percent={m.memory_usage_percent}
              detail={`${m.memory_used_gb} / ${m.memory_total_gb} GB`}
            />
            <MetricsBar
              label="Disk"
              percent={m.disk_usage_percent}
              detail={`${m.disk_used_gb} / ${m.disk_total_gb} GB`}
            />
            <div className="text-xs text-muted-foreground">
              Load: {m.load_avg_1m} / {m.load_avg_5m} / {m.load_avg_15m}
            </div>
          </div>
        )}

        {/* Processes divider + list */}
        {node.workers.length > 0 && (
          <div>
            <div className="text-[10px] uppercase tracking-widest text-muted-foreground border-t pt-2 mb-1">
              Processes
            </div>
            {node.workers.map((w) => {
              const search = node.searches.find((s) => s.worker_id === w.worker_id) ?? null;
              return (
                <div
                  key={w.worker_id}
                  className={onInspectWorker ? "cursor-pointer hover:bg-muted/50 rounded-sm transition-colors" : ""}
                  onClick={() => onInspectWorker?.(w)}
                >
                  <ProcessRow worker={w} search={search} />
                </div>
              );
            })}
          </div>
        )}

        {/* Deployments */}
        {node.deployments.length > 0 && (
          <div>
            <div className="text-[10px] uppercase tracking-widest text-muted-foreground border-t pt-2 mb-1">
              Deployments
            </div>
            {node.deployments.map((d) => (
              <div
                key={d.id}
                className="flex items-center gap-2 text-xs text-muted-foreground py-1 ml-2 pl-3 border-l-2 border-muted-foreground/20"
              >
                <div
                  className={`size-1.5 rounded-full flex-shrink-0 ${
                    d.status === "running"
                      ? "bg-green-500"
                      : d.status === "deploying"
                        ? "bg-yellow-500 animate-pulse"
                        : d.status === "failed"
                          ? "bg-red-500"
                          : "bg-muted-foreground"
                  }`}
                />
                <span className="font-medium text-foreground">
                  {d.ssh_user}@{d.hostname}
                </span>
                <Badge variant="outline" className="text-[10px] px-1.5 py-0">{d.search_type}</Badge>
                <span className="capitalize ml-auto">{d.status}</span>
                {d.error && (
                  <span className="text-red-500 truncate max-w-[150px]" title={d.error}>
                    {d.error}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
