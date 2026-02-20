"use client";

import { useMemo } from "react";
import { cn } from "@/lib/cn";
import { generateUptimeDays, type ServiceStatus } from "@/lib/status-data";

const barColors: Record<ServiceStatus, string> = {
  operational: "bg-accent-green",
  degraded: "bg-accent-orange",
  down: "bg-destructive",
};

interface UptimeBarProps {
  label: string;
  days?: number;
}

export function UptimeBar({ label, days = 90 }: UptimeBarProps) {
  const uptimeDays = useMemo(() => generateUptimeDays(days), [days]);

  const operationalCount = uptimeDays.filter(
    (d) => d.status === "operational"
  ).length;
  const uptimePercent = ((operationalCount / uptimeDays.length) * 100).toFixed(
    2
  );

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm text-text font-medium">{label}</span>
        <span className="text-sm text-text-muted">{uptimePercent}% uptime</span>
      </div>
      <div className="flex gap-px">
        {uptimeDays.map((day) => (
          <div
            key={day.date}
            className={cn(
              "flex-1 h-8 rounded-sm first:rounded-l-md last:rounded-r-md",
              barColors[day.status]
            )}
            title={`${day.date}: ${day.status}`}
          />
        ))}
      </div>
      <div className="flex justify-between mt-1">
        <span className="text-xs text-text-muted">{days} days ago</span>
        <span className="text-xs text-text-muted">Today</span>
      </div>
    </div>
  );
}
