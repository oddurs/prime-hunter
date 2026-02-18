/**
 * @module service-status-card
 *
 * Small status indicator card for external services (Supabase, WebSocket,
 * Coordinator). Shows a colored dot (green/amber/red) with a label and
 * optional detail text. Used in the dashboard header area.
 */

import type { ReactNode } from "react";
import { Card, CardContent } from "@/components/ui/card";

type ServiceStatus = "online" | "degraded" | "offline";

const dotColor: Record<ServiceStatus, string> = {
  online: "bg-green-500",
  degraded: "bg-yellow-500",
  offline: "bg-muted-foreground",
};

interface ServiceStatusCardProps {
  name: string;
  status: ServiceStatus;
  children?: ReactNode;
}

export function ServiceStatusCard({ name, status, children }: ServiceStatusCardProps) {
  return (
    <Card className="py-3">
      <CardContent className="p-0 px-4 space-y-2">
        <div className="flex items-center gap-2">
          <div className={`size-2 rounded-full flex-shrink-0 ${dotColor[status]}`} />
          <span className="text-sm font-semibold text-foreground">{name}</span>
          <span className="text-[10px] text-muted-foreground ml-auto capitalize">{status}</span>
        </div>
        {children}
      </CardContent>
    </Card>
  );
}
