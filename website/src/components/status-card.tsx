import type { Service, ServiceStatus } from "@/lib/status-data";
import { cn } from "@/lib/cn";

const statusColors: Record<ServiceStatus, string> = {
  operational: "bg-accent-green",
  degraded: "bg-accent-orange",
  down: "bg-destructive",
};

const statusLabels: Record<ServiceStatus, string> = {
  operational: "Operational",
  degraded: "Degraded",
  down: "Down",
};

export function StatusCard({ service }: { service: Service }) {
  return (
    <div className="flex items-center justify-between p-4 rounded-md border border-border bg-card">
      <div>
        <div className="flex items-center gap-2 mb-1">
          <span
            className={cn(
              "inline-block w-2.5 h-2.5 rounded-full",
              statusColors[service.status],
              service.status === "operational" && "pulse-green"
            )}
          />
          <h3 className="text-foreground font-semibold">{service.name}</h3>
        </div>
        <p className="text-sm text-muted-foreground">{service.description}</p>
      </div>
      <div className="text-right shrink-0 ml-4">
        <span
          className={cn(
            "text-sm font-medium",
            service.status === "operational"
              ? "text-accent-green"
              : service.status === "degraded"
                ? "text-accent-orange"
                : "text-destructive"
          )}
        >
          {statusLabels[service.status]}
        </span>
        {service.latency && (
          <p className="text-xs text-muted-foreground mt-0.5">{service.latency}</p>
        )}
      </div>
    </div>
  );
}
