/**
 * @module metrics-bar
 *
 * Horizontal progress bar for hardware metrics (CPU, memory, disk).
 * Renders a colored fill bar that transitions from green → amber → red
 * as utilization increases. Used inside `HostNodeCard` and `AgentControllerCard`.
 */

interface MetricsBarProps {
  label: string;
  percent: number;
  detail?: string;
}

export function MetricsBar({ label, percent, detail }: MetricsBarProps) {
  const clamped = Math.min(100, Math.max(0, percent));
  const color =
    clamped >= 90
      ? "bg-red-500"
      : clamped >= 70
        ? "bg-yellow-500"
        : "bg-green-500";

  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs text-muted-foreground">
        <span>{label}</span>
        <span className="tabular-nums">
          {detail ?? `${clamped.toFixed(1)}%`}
        </span>
      </div>
      <div className="h-1.5 bg-muted rounded-full overflow-hidden">
        <div
          className={`h-full ${color} rounded-full transition-all duration-1000`}
          style={{ width: `${clamped}%` }}
        />
      </div>
    </div>
  );
}
