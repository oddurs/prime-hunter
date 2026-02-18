/**
 * @module agent-controller-card
 *
 * Dashboard card for the Claude Code agent subsystem. Shows running
 * agents, recent task summaries, and API budget usage. Provides a
 * compact view of autonomous agent activity without navigating to
 * the full Agents page.
 */

import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { MetricsBar } from "@/components/metrics-bar";
import type { AgentInfo, AgentTaskSummary, AgentBudgetSummary } from "@/hooks/use-websocket";

interface AgentControllerCardProps {
  runningAgents: AgentInfo[];
  agentTasks: AgentTaskSummary[];
  agentBudgets: AgentBudgetSummary[];
}

function elapsed(startedAt: string): string {
  const ms = Date.now() - new Date(startedAt).getTime();
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  return `${hrs}h ${mins % 60}m`;
}

export function AgentControllerCard({
  runningAgents,
  agentTasks,
  agentBudgets,
}: AgentControllerCardProps) {
  const hasActivity = runningAgents.length > 0 || agentTasks.some((t) => t.status === "pending" || t.status === "running");
  const statusDot = hasActivity ? "bg-green-500" : "bg-muted-foreground";
  const statusLabel = hasActivity ? "active" : "idle";

  // Latest budget period
  const budget = agentBudgets.length > 0 ? agentBudgets[agentBudgets.length - 1] : null;
  const budgetPct = budget && budget.budget_usd > 0
    ? (budget.spent_usd / budget.budget_usd) * 100
    : 0;

  // Task counts
  const running = agentTasks.filter((t) => t.status === "running").length;
  const pending = agentTasks.filter((t) => t.status === "pending").length;
  const completed = agentTasks.filter((t) => t.status === "completed").length;

  return (
    <Card className="py-3">
      <CardContent className="p-0 px-4 space-y-2">
        <div className="flex items-center gap-2">
          <div className={`size-2 rounded-full flex-shrink-0 ${statusDot}`} />
          <span className="text-sm font-semibold text-foreground">Agent Controller</span>
          {runningAgents.length > 0 && (
            <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
              {runningAgents.length} running
            </Badge>
          )}
          <span className="text-[10px] text-muted-foreground ml-auto capitalize">{statusLabel}</span>
        </div>

        {budget && (
          <MetricsBar
            label="Budget"
            percent={budgetPct}
            detail={`$${budget.spent_usd.toFixed(2)} / $${budget.budget_usd.toFixed(2)}`}
          />
        )}

        {agentTasks.length > 0 && (
          <div className="text-xs text-muted-foreground">
            {running > 0 && <span className="text-foreground font-medium">{running} running</span>}
            {running > 0 && (pending > 0 || completed > 0) && ", "}
            {pending > 0 && `${pending} pending`}
            {pending > 0 && completed > 0 && ", "}
            {completed > 0 && `${completed} completed`}
          </div>
        )}

        {runningAgents.length > 0 && (
          <div className="space-y-1 border-t pt-2">
            {runningAgents.map((agent) => (
              <div key={agent.task_id} className="flex items-center gap-2 text-xs">
                <Badge variant="outline" className="text-[10px] px-1.5 py-0 font-mono">
                  {agent.model}
                </Badge>
                <span className="text-muted-foreground truncate flex-1" title={agent.title}>
                  {agent.title}
                </span>
                <span className="text-muted-foreground tabular-nums flex-shrink-0">
                  {elapsed(agent.started_at)}
                </span>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
