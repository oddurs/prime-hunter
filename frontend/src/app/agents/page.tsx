"use client";

/**
 * @module agents/page
 *
 * Claude Code agent management page. Allows creating, monitoring,
 * and cancelling autonomous agent tasks. Agents run as `claude`
 * subprocesses on the coordinator, working on engine, frontend,
 * or infrastructure tasks with full codebase context.
 *
 * Supports template-based workflows (multi-step task trees) and
 * custom single tasks. Displays task hierarchy, dependency status,
 * live output streaming, and budget tracking.
 *
 * Components are split into `components/agents/` for maintainability:
 * - helpers.tsx: shared badges, icons, constants
 * - new-task-dialog.tsx: task creation dialog (role/template/custom)
 * - task-card.tsx: task card with subtask tree, event log, detail dialog
 * - activity-feed.tsx: real-time event feed
 * - budget-cards.tsx: budget period management
 * - memory-tab.tsx: agent knowledge store
 * - schedules-tab.tsx: automated scheduling (cron/event)
 * - analytics-tab.tsx: cost analytics and anomaly detection
 */

import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useAgentTasks,
  useAgentBudgets,
  useAgentRoles,
  buildTaskTree,
  type TaskTreeNode,
} from "@/hooks/use-agents";
import { ViewHeader } from "@/components/view-header";
import { StatCard } from "@/components/stat-card";
import { EmptyState } from "@/components/empty-state";
import {
  Bot,
  CheckCircle2,
  DollarSign,
  ListTodo,
  Plus,
} from "lucide-react";
import { ROLE_CONFIG } from "@/components/agents/helpers";
import { NewTaskDialog } from "@/components/agents/new-task-dialog";
import { TaskCard } from "@/components/agents/task-card";
import { ActivityFeed } from "@/components/agents/activity-feed";
import { BudgetCards } from "@/components/agents/budget-cards";
import { MemoryTab } from "@/components/agents/memory-tab";
import { SchedulesTab } from "@/components/agents/schedules-tab";
import { AnalyticsTab } from "@/components/agents/analytics-tab";

export default function AgentsPage() {
  const { tasks } = useAgentTasks();
  const { budgets } = useAgentBudgets();
  const { roles } = useAgentRoles();
  const [newTaskOpen, setNewTaskOpen] = useState(false);
  const [filter, setFilter] = useState("all");
  const [roleFilter, setRoleFilter] = useState<string | null>(null);

  const taskTree = useMemo(() => buildTaskTree(tasks), [tasks]);

  const counts = useMemo(() => {
    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    return {
      active: tasks.filter((t) => t.status === "in_progress").length,
      pending: tasks.filter((t) => t.status === "pending").length,
      completedToday: tasks.filter(
        (t) => t.status === "completed" && t.completed_at && new Date(t.completed_at) >= todayStart
      ).length,
    };
  }, [tasks]);

  const dailyBudget = budgets.find((b) => b.period === "daily");
  const todaySpend = dailyBudget?.spent_usd ?? 0;
  const todayLimit = dailyBudget?.budget_usd ?? 0;

  const filteredTree = useMemo(() => {
    let result = taskTree;
    if (filter !== "all") {
      result = result.filter((node) => {
        if (node.task.status === filter) return true;
        if (node.children.some((c) => c.status === filter)) return true;
        return false;
      });
    }
    if (roleFilter) {
      result = result.filter((node) => {
        if (node.task.role_name === roleFilter) return true;
        if (node.children.some((c) => c.role_name === roleFilter)) return true;
        return false;
      });
    }
    return result;
  }, [taskTree, filter, roleFilter]);

  return (
    <>
      <Tabs defaultValue="tasks">
        <ViewHeader
          title="Agents"
          subtitle={`${counts.active} running \u00b7 ${counts.pending} queued`}
          actions={
            <Button size="sm" onClick={() => setNewTaskOpen(true)}>
              <Plus className="size-4 mr-1" />
              New Task
            </Button>
          }
          tabs={
            <TabsList variant="line">
              <TabsTrigger value="tasks">
                Tasks{tasks.length > 0 ? ` (${tasks.length})` : ""}
              </TabsTrigger>
              <TabsTrigger value="activity">Activity</TabsTrigger>
              <TabsTrigger value="memory">Memory</TabsTrigger>
              <TabsTrigger value="budget">Budget</TabsTrigger>
              <TabsTrigger value="schedules">Schedules</TabsTrigger>
              <TabsTrigger value="analytics">Analytics</TabsTrigger>
            </TabsList>
          }
        />

        {/* Metric Cards */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4">
          <StatCard label="Active Tasks" value={counts.active} icon={<Bot className="size-4 text-primary" />} />
          <StatCard label="Pending Queue" value={counts.pending} icon={<ListTodo className="size-4 text-amber-500" />} />
          <StatCard label="Completed Today" value={counts.completedToday} icon={<CheckCircle2 className="size-4 text-green-500" />} />
          <StatCard
            label="Today's Spend"
            value={<>
              ${todaySpend.toFixed(2)}
              {todayLimit > 0 && (
                <span className="text-xs font-normal text-muted-foreground ml-1">
                  / ${todayLimit.toFixed(2)}
                </span>
              )}
            </>}
            icon={<DollarSign className="size-4 text-green-600" />}
          />
        </div>

        {/* Tasks Tab */}
        <TabsContent value="tasks" className="mt-4">
          <div className="flex flex-wrap items-center gap-1.5 mb-3">
            {["all", "pending", "in_progress", "completed", "failed"].map((f) => (
              <Button
                key={f}
                variant={filter === f ? "default" : "outline"}
                size="xs"
                onClick={() => setFilter(f)}
              >
                {f === "all"
                  ? "All"
                  : f === "in_progress"
                    ? "Running"
                    : f.charAt(0).toUpperCase() + f.slice(1)}
              </Button>
            ))}
            {roles.length > 0 && (
              <>
                <span className="text-xs text-muted-foreground mx-1">|</span>
                <Button
                  variant={roleFilter === null ? "default" : "outline"}
                  size="xs"
                  onClick={() => setRoleFilter(null)}
                >
                  Any Role
                </Button>
                {roles.map((r) => {
                  const cfg = ROLE_CONFIG[r.name];
                  return (
                    <Button
                      key={r.name}
                      variant={roleFilter === r.name ? "default" : "outline"}
                      size="xs"
                      onClick={() => setRoleFilter(r.name)}
                    >
                      {cfg?.label || r.name}
                    </Button>
                  );
                })}
              </>
            )}
          </div>

          {filteredTree.length === 0 ? (
            <EmptyState
              message={
                filter === "all"
                  ? 'No tasks yet. Click "New Task" to create one.'
                  : `No ${filter === "in_progress" ? "running" : filter} tasks.`
              }
            />
          ) : (
            <div className="space-y-2">
              {filteredTree.map((node: TaskTreeNode) => (
                <TaskCard key={node.task.id} task={node.task} children={node.children} />
              ))}
            </div>
          )}
        </TabsContent>

        {/* Activity Tab */}
        <TabsContent value="activity" className="mt-4">
          <ActivityFeed />
        </TabsContent>

        {/* Memory Tab */}
        <TabsContent value="memory" className="mt-4">
          <MemoryTab />
        </TabsContent>

        {/* Budget Tab */}
        <TabsContent value="budget" className="mt-4">
          <BudgetCards />
        </TabsContent>

        {/* Schedules Tab */}
        <TabsContent value="schedules" className="mt-4">
          <SchedulesTab />
        </TabsContent>

        {/* Analytics Tab */}
        <TabsContent value="analytics" className="mt-4">
          <AnalyticsTab />
        </TabsContent>
      </Tabs>

      <NewTaskDialog open={newTaskOpen} onOpenChange={setNewTaskOpen} />
    </>
  );
}
