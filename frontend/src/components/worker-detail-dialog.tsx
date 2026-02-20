import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { MetricsBar } from "@/components/metrics-bar";
import { JsonBlock } from "@/components/json-block";
import { numberWithCommas, formatUptime } from "@/lib/format";
import type { WorkerStatus } from "@/hooks/use-websocket";

function parseJson(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object") return null;
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}

interface WorkerDetailDialogProps {
  worker: WorkerStatus | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function WorkerDetailDialog({ worker, open, onOpenChange }: WorkerDetailDialogProps) {
  if (!worker) {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle>Worker</DialogTitle>
          </DialogHeader>
        </DialogContent>
      </Dialog>
    );
  }

  const throughput = worker.uptime_secs > 0
    ? (worker.tested / worker.uptime_secs).toFixed(1)
    : "0.0";
  const params = parseJson(worker.search_params);
  const checkpoint = worker.checkpoint ? parseJson(worker.checkpoint) : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <div
              className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${
                worker.last_heartbeat_secs_ago < 30
                  ? "bg-green-500"
                  : worker.last_heartbeat_secs_ago < 60
                    ? "bg-yellow-500"
                    : "bg-red-500"
              }`}
            />
            {worker.hostname}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-4 text-sm">
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Worker ID</div>
              <span className="font-mono text-xs">{worker.worker_id}</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Search Type</div>
              <Badge variant="outline">{worker.search_type}</Badge>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Cores</div>
              <span className="font-semibold">{worker.cores}</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Uptime</div>
              <span>{formatUptime(worker.uptime_secs)}</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Tested</div>
              <span className="font-semibold">{numberWithCommas(worker.tested)}</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Found</div>
              <span className="font-semibold">{worker.found}</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Throughput</div>
              <span>{throughput} candidates/sec</span>
            </div>
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Heartbeat</div>
              <span>
                {worker.last_heartbeat_secs_ago < 5
                  ? "just now"
                  : `${worker.last_heartbeat_secs_ago}s ago`}
              </span>
            </div>
          </div>
          {worker.metrics && (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-2">Hardware</div>
              <div className="space-y-2">
                <MetricsBar label="CPU" percent={worker.metrics.cpu_usage_percent} />
                <MetricsBar
                  label="Memory"
                  percent={worker.metrics.memory_usage_percent}
                  detail={`${worker.metrics.memory_used_gb} / ${worker.metrics.memory_total_gb} GB`}
                />
                <MetricsBar
                  label="Disk"
                  percent={worker.metrics.disk_usage_percent}
                  detail={`${worker.metrics.disk_used_gb} / ${worker.metrics.disk_total_gb} GB`}
                />
                <div className="text-xs text-muted-foreground">
                  Load: {worker.metrics.load_avg_1m} / {worker.metrics.load_avg_5m} / {worker.metrics.load_avg_15m}
                </div>
              </div>
            </div>
          )}
          {worker.current && (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">Current candidate</div>
              <div className="font-mono text-xs break-all bg-muted rounded-md p-2">
                {worker.current}
              </div>
            </div>
          )}
          {params && <JsonBlock label="Search parameters" data={params} />}
          {checkpoint && <JsonBlock label="Checkpoint" data={checkpoint} />}
        </div>
      </DialogContent>
    </Dialog>
  );
}
