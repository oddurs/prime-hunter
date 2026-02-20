/**
 * @module agents/helpers
 *
 * Shared constants, badge renderers, and utility functions used across
 * agent management components. Centralizes status display logic, role
 * configuration, and cost estimation heuristics.
 */

import { Badge } from "@/components/ui/badge";
import {
  Bot,
  Plus,
  Play,
  CheckCircle2,
  XCircle,
  MessageSquare,
  Wrench,
  ArrowRight,
  AlertCircle,
  Stethoscope,
  DollarSign,
  Cog,
  Layout,
  Server,
  BookOpen,
} from "lucide-react";

// --- Status badge ---

/** Renders a colored Badge for task status (pending, running, completed, failed, cancelled). */
export function statusBadge(status: string) {
  const variants: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
    pending: "outline",
    in_progress: "default",
    completed: "secondary",
    failed: "destructive",
    cancelled: "outline",
  };
  const labels: Record<string, string> = {
    pending: "Pending",
    in_progress: "Running",
    completed: "Completed",
    failed: "Failed",
    cancelled: "Cancelled",
  };
  return <Badge variant={variants[status] || "outline"}>{labels[status] || status}</Badge>;
}

/** Renders a colored text label for task priority (low, normal, high, urgent). */
export function priorityBadge(priority: string) {
  const colors: Record<string, string> = {
    low: "text-muted-foreground",
    normal: "text-foreground",
    high: "text-amber-500",
    urgent: "text-red-500",
  };
  return (
    <span className={`text-xs font-medium ${colors[priority] || ""}`}>
      {priority}
    </span>
  );
}

/** Returns a small colored icon for agent event types. */
export function eventIcon(type: string) {
  switch (type) {
    case "created":
      return <Plus className="size-3.5 text-blue-500" />;
    case "started":
      return <Play className="size-3.5 text-green-500" />;
    case "completed":
    case "parent_completed":
      return <CheckCircle2 className="size-3.5 text-green-500" />;
    case "failed":
    case "parent_failed":
      return <XCircle className="size-3.5 text-red-500" />;
    case "cancelled":
      return <XCircle className="size-3.5 text-muted-foreground" />;
    case "message":
      return <MessageSquare className="size-3.5 text-blue-400" />;
    case "tool_call":
      return <Wrench className="size-3.5 text-purple-500" />;
    case "tool_result":
      return <ArrowRight className="size-3.5 text-purple-400" />;
    case "error":
      return <AlertCircle className="size-3.5 text-red-500" />;
    case "diagnosis":
      return <Stethoscope className="size-3.5 text-amber-500" />;
    case "budget_exceeded":
      return <DollarSign className="size-3.5 text-red-500" />;
    default:
      return <Bot className="size-3.5 text-muted-foreground" />;
  }
}

/** Small colored dot indicating task status. */
export function statusDot(status: string) {
  const colors: Record<string, string> = {
    pending: "bg-muted-foreground/30",
    in_progress: "bg-blue-500",
    completed: "bg-green-500",
    failed: "bg-red-500",
    cancelled: "bg-muted-foreground/30",
  };
  return <span className={`size-2 rounded-full shrink-0 ${colors[status] || "bg-muted"}`} />;
}

// --- Role configuration ---

/** Cost rate estimates ($/min) matching server-side rates. */
export const MODEL_RATES: Record<string, { perMin: number; label: string }> = {
  opus: { perMin: 0.90, label: "Opus" },
  sonnet: { perMin: 0.30, label: "Sonnet" },
  haiku: { perMin: 0.06, label: "Haiku" },
};

/** Role display configuration: icon, color, and label. */
export const ROLE_CONFIG: Record<string, { icon: typeof Cog; color: string; label: string }> = {
  engine: { icon: Cog, color: "amber", label: "Engine" },
  frontend: { icon: Layout, color: "blue", label: "Frontend" },
  ops: { icon: Server, color: "green", label: "Ops" },
  research: { icon: BookOpen, color: "purple", label: "Research" },
};

/** Renders a small colored Badge for an agent role name. */
export function roleBadge(roleName: string | null) {
  if (!roleName) return null;
  const cfg = ROLE_CONFIG[roleName];
  if (!cfg) return <Badge variant="outline" className="text-[10px] px-1.5 py-0">{roleName}</Badge>;
  const colorMap: Record<string, string> = {
    amber: "border-amber-400 text-amber-600 dark:text-amber-400",
    blue: "border-blue-400 text-blue-600 dark:text-blue-400",
    green: "border-green-400 text-green-600 dark:text-green-400",
    purple: "border-purple-400 text-purple-600 dark:text-purple-400",
  };
  return (
    <Badge variant="outline" className={`text-[10px] px-1.5 py-0 ${colorMap[cfg.color] || ""}`}>
      {cfg.label}
    </Badge>
  );
}

/** Heuristic cost estimate based on model rate and description length. */
export function estimateCostRange(model: string, descriptionLength: number) {
  const rate = MODEL_RATES[model] || MODEL_RATES.sonnet;
  const minMinutes = descriptionLength > 200 ? 3 : 1;
  const maxMinutes = descriptionLength > 500 ? 15 : descriptionLength > 200 ? 8 : 4;
  return {
    low: rate.perMin * minMinutes,
    high: rate.perMin * maxMinutes,
  };
}
