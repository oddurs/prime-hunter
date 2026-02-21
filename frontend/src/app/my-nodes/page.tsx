"use client";

import { useEffect, useState } from "react";
import { Server, Cpu, HardDrive, Wifi, WifiOff } from "lucide-react";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ViewHeader } from "@/components/view-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import { useAuth } from "@/contexts/auth-context";
import { API_BASE, relativeTime } from "@/lib/format";

interface NodeRow {
  worker_id: string;
  hostname: string | null;
  cores: number | null;
  cpu_model: string | null;
  os: string | null;
  arch: string | null;
  ram_gb: number | null;
  has_gpu: boolean | null;
  gpu_model: string | null;
  worker_version: string | null;
  registered_at: string;
  last_heartbeat: string | null;
}

/** Returns true if the node's last heartbeat is within 2 minutes. */
function isOnline(node: NodeRow): boolean {
  if (!node.last_heartbeat) return false;
  const diff = Date.now() - new Date(node.last_heartbeat).getTime();
  return diff < 2 * 60 * 1000;
}

export default function MyNodesPage() {
  const { session } = useAuth();
  const [nodes, setNodes] = useState<NodeRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!session?.access_token) return;

    async function fetchNodes() {
      setLoading(true);
      setError(null);
      try {
        const res = await fetch(`${API_BASE}/api/v1/operators/me/nodes`, {
          headers: {
            Authorization: `Bearer ${session!.access_token}`,
          },
        });
        if (!res.ok) {
          const data = await res.json().catch(() => ({}));
          throw new Error(data.error || `HTTP ${res.status}`);
        }
        const data: NodeRow[] = await res.json();
        setNodes(data);
      } catch (err) {
        const message =
          err instanceof Error ? err.message : "Failed to fetch nodes";
        setError(message);
      } finally {
        setLoading(false);
      }
    }

    void fetchNodes();
  }, [session]);

  const totalNodes = nodes.length;
  const totalCores = nodes.reduce((sum, n) => sum + (n.cores ?? 0), 0);
  const onlineNodes = nodes.filter(isOnline).length;

  return (
    <>
      <ViewHeader
        title="My Nodes"
        subtitle="Monitor your registered compute nodes and their status."
        className="mb-5"
      />

      <div className="grid grid-cols-2 lg:grid-cols-3 gap-3 mb-5">
        <StatCard
          label="Total Nodes"
          value={totalNodes}
          icon={<Server className="h-4 w-4 text-muted-foreground" />}
        />
        <StatCard
          label="Total Cores"
          value={totalCores}
          icon={<Cpu className="h-4 w-4 text-muted-foreground" />}
        />
        <StatCard
          label="Online Nodes"
          value={onlineNodes}
          icon={<Wifi className="h-4 w-4 text-emerald-500" />}
        />
      </div>

      {loading && (
        <div className="text-sm text-muted-foreground py-8 text-center">
          Loading nodes...
        </div>
      )}

      {error && (
        <div className="text-sm text-destructive py-8 text-center">
          {error}
        </div>
      )}

      {!loading && !error && nodes.length === 0 && (
        <EmptyState message="No nodes registered yet. Run the darkreach worker to register your first node." />
      )}

      {!loading && !error && nodes.length > 0 && (
        <Card className="rounded-md shadow-none">
          <CardHeader className="pb-2">
            <CardTitle className="text-base">Registered Nodes</CardTitle>
          </CardHeader>
          <CardContent>
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-xs font-medium text-muted-foreground">Hostname</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Worker ID</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Cores</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">CPU</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">OS/Arch</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Version</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Status</TableHead>
                  <TableHead className="text-xs font-medium text-muted-foreground">Last Seen</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {nodes.map((node) => {
                  const online = isOnline(node);
                  return (
                    <TableRow key={node.worker_id} className="hover:bg-muted/30">
                      <TableCell className="font-medium">
                        {node.hostname ?? "-"}
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {node.worker_id}
                      </TableCell>
                      <TableCell>{node.cores ?? "-"}</TableCell>
                      <TableCell className="text-muted-foreground text-xs max-w-48 truncate">
                        {node.cpu_model ?? "-"}
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">
                        {node.os && node.arch
                          ? `${node.os} / ${node.arch}`
                          : node.os ?? node.arch ?? "-"}
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {node.worker_version ?? "-"}
                      </TableCell>
                      <TableCell>
                        {online ? (
                          <Badge className="bg-emerald-500/15 text-emerald-500 border-emerald-500/25">
                            <Wifi className="h-3 w-3 mr-1" />
                            Online
                          </Badge>
                        ) : (
                          <Badge variant="outline" className="text-red-500 border-red-500/25">
                            <WifiOff className="h-3 w-3 mr-1" />
                            Offline
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell className="text-muted-foreground text-xs">
                        {node.last_heartbeat
                          ? relativeTime(node.last_heartbeat)
                          : "Never"}
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </>
  );
}
