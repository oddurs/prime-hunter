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
 */

import { useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  useAgentTasks,
  useAgentEvents,
  useAgentBudgets,
  useAgentMemory,
  useAgentTemplates,
  useAgentRoles,
  createTask,
  cancelTask,
  expandTemplate,
  buildTaskTree,
  upsertMemory,
  deleteMemory,
  MEMORY_CATEGORIES,
  type AgentTask,
  type AgentEvent,
  type AgentRole,
  type AgentMemory,
  type MemoryCategory,
  type TaskTreeNode,
} from "@/hooks/use-agents";
import { ViewHeader } from "@/components/view-header";
import { numberWithCommas, formatTime } from "@/lib/format";
import {
  Bot,
  Clock,
  CheckCircle2,
  DollarSign,
  ListTodo,
  MessageSquare,
  AlertCircle,
  XCircle,
  Play,
  Wrench,
  Plus,
  Pencil,
  Brain,
  Trash2,
  ChevronDown,
  ChevronRight,
  GitBranch,
  LayoutTemplate,
  Cog,
  Layout,
  Server,
  BookOpen,
} from "lucide-react";

// --- Status badge ---

function statusBadge(status: string) {
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

function priorityBadge(priority: string) {
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

function eventIcon(type: string) {
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
    case "error":
      return <AlertCircle className="size-3.5 text-red-500" />;
    default:
      return <Bot className="size-3.5 text-muted-foreground" />;
  }
}

// --- New Task Dialog ---

// Cost rate estimates ($/min) matching server-side rates
const MODEL_RATES: Record<string, { perMin: number; label: string }> = {
  opus: { perMin: 0.90, label: "Opus" },
  sonnet: { perMin: 0.30, label: "Sonnet" },
  haiku: { perMin: 0.06, label: "Haiku" },
};

/** Role display configuration: icon, color, and label. */
const ROLE_CONFIG: Record<string, { icon: typeof Cog; color: string; label: string }> = {
  engine: { icon: Cog, color: "amber", label: "Engine" },
  frontend: { icon: Layout, color: "blue", label: "Frontend" },
  ops: { icon: Server, color: "green", label: "Ops" },
  research: { icon: BookOpen, color: "purple", label: "Research" },
};

function roleBadge(roleName: string | null) {
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

function estimateCostRange(model: string, descriptionLength: number) {
  const rate = MODEL_RATES[model] || MODEL_RATES.sonnet;
  // Heuristic: short tasks ~2min, long tasks ~10min
  const minMinutes = descriptionLength > 200 ? 3 : 1;
  const maxMinutes = descriptionLength > 500 ? 15 : descriptionLength > 200 ? 8 : 4;
  return {
    low: rate.perMin * minMinutes,
    high: rate.perMin * maxMinutes,
  };
}

function NewTaskDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { budgets } = useAgentBudgets();
  const { templates } = useAgentTemplates();
  const { roles } = useAgentRoles();
  const [mode, setMode] = useState<"role" | "template" | "custom">("role");
  const [selectedRole, setSelectedRole] = useState<string | null>(null);
  const [selectedTemplate, setSelectedTemplate] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("normal");
  const [model, setModel] = useState("");
  const [maxCost, setMaxCost] = useState("");
  const [permissionLevel, setPermissionLevel] = useState(1);
  const [submitting, setSubmitting] = useState(false);

  // When a role is selected, apply its defaults
  const activeRole = roles.find((r) => r.name === selectedRole);

  const effectiveModel = model || activeRole?.default_model || "sonnet";
  const costEstimate = useMemo(() => {
    return estimateCostRange(effectiveModel, description.length);
  }, [effectiveModel, description.length]);

  const dailyBudget = budgets.find((b) => b.period === "daily");
  const remaining = dailyBudget ? dailyBudget.budget_usd - dailyBudget.spent_usd : null;
  const highEstimateWarning =
    remaining !== null && costEstimate.high > remaining && remaining > 0;

  // Show role-filtered templates when a role is selected, otherwise show all
  const filteredTemplates = useMemo(() => {
    if (mode === "role" && selectedRole) {
      return templates.filter(
        (t) => t.role_name === selectedRole || t.role_name === null
      );
    }
    return templates;
  }, [templates, mode, selectedRole]);

  const activeTemplate = templates.find((t) => t.name === selectedTemplate);

  function resetForm() {
    setTitle("");
    setDescription("");
    setPriority("normal");
    setModel("");
    setMaxCost("");
    setPermissionLevel(1);
    setSelectedTemplate(null);
    setSelectedRole(null);
  }

  function handleRoleSelect(roleName: string) {
    setSelectedRole(roleName);
    const role = roles.find((r) => r.name === roleName);
    if (role) {
      setPermissionLevel(role.default_permission_level);
      setModel(role.default_model);
      if (role.default_max_cost_usd != null) {
        setMaxCost(role.default_max_cost_usd.toString());
      }
    }
    setSelectedTemplate(null);
  }

  async function handleSubmit() {
    if (!title.trim()) return;
    setSubmitting(true);
    try {
      const parsedMaxCost = maxCost ? parseFloat(maxCost) : undefined;
      const roleName = (mode === "role" && selectedRole) ? selectedRole : undefined;

      if ((mode === "template" || mode === "role") && selectedTemplate) {
        await expandTemplate(
          selectedTemplate,
          title.trim(),
          description.trim(),
          priority,
          parsedMaxCost && !isNaN(parsedMaxCost) ? parsedMaxCost : undefined,
          permissionLevel,
          roleName
        );
        toast.success(`Template "${selectedTemplate}" expanded`);
      } else {
        await createTask(
          title.trim(),
          description.trim(),
          priority,
          model || undefined,
          parsedMaxCost && !isNaN(parsedMaxCost) ? parsedMaxCost : undefined,
          permissionLevel,
          roleName
        );
        toast.success("Task created");
      }
      resetForm();
      onOpenChange(false);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create task");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>New Agent Task</DialogTitle>
        </DialogHeader>
        <div className="space-y-3">
          {/* 3-mode toggle */}
          <div className="grid grid-cols-3 gap-2">
            <button
              type="button"
              onClick={() => setMode("role")}
              className={`rounded-md border px-3 py-2 text-sm text-left transition-colors ${
                mode === "role"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:border-foreground/30"
              }`}
            >
              <div className="flex items-center gap-2">
                <Cog className="size-4" />
                <div>
                  <div className="font-medium">Role</div>
                  <div className="text-[10px] opacity-70">Domain preset</div>
                </div>
              </div>
            </button>
            <button
              type="button"
              onClick={() => setMode("template")}
              className={`rounded-md border px-3 py-2 text-sm text-left transition-colors ${
                mode === "template"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:border-foreground/30"
              }`}
            >
              <div className="flex items-center gap-2">
                <LayoutTemplate className="size-4" />
                <div>
                  <div className="font-medium">Template</div>
                  <div className="text-[10px] opacity-70">Multi-step</div>
                </div>
              </div>
            </button>
            <button
              type="button"
              onClick={() => setMode("custom")}
              className={`rounded-md border px-3 py-2 text-sm text-left transition-colors ${
                mode === "custom"
                  ? "border-primary bg-primary/10 text-primary"
                  : "border-border text-muted-foreground hover:border-foreground/30"
              }`}
            >
              <div className="flex items-center gap-2">
                <Bot className="size-4" />
                <div>
                  <div className="font-medium">Custom</div>
                  <div className="text-[10px] opacity-70">Single task</div>
                </div>
              </div>
            </button>
          </div>

          {/* Role selector */}
          {mode === "role" && (
            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground">Select Role</label>
              <div className="grid grid-cols-2 gap-2">
                {roles.map((role) => {
                  const cfg = ROLE_CONFIG[role.name] || { icon: Bot, color: "gray", label: role.name };
                  const Icon = cfg.icon;
                  const colorClasses: Record<string, string> = {
                    amber: "border-amber-500 bg-amber-500/10 text-amber-700 dark:text-amber-400",
                    blue: "border-blue-500 bg-blue-500/10 text-blue-700 dark:text-blue-400",
                    green: "border-green-500 bg-green-500/10 text-green-700 dark:text-green-400",
                    purple: "border-purple-500 bg-purple-500/10 text-purple-700 dark:text-purple-400",
                  };
                  const isActive = selectedRole === role.name;
                  return (
                    <button
                      key={role.name}
                      type="button"
                      onClick={() => handleRoleSelect(role.name)}
                      className={`rounded-md border px-3 py-2 text-left transition-colors ${
                        isActive
                          ? colorClasses[cfg.color] || "border-primary bg-primary/10"
                          : "border-border text-muted-foreground hover:border-foreground/30"
                      }`}
                    >
                      <div className="flex items-center gap-2">
                        <Icon className="size-4 shrink-0" />
                        <div className="min-w-0">
                          <div className="font-medium text-sm capitalize">{cfg.label}</div>
                          <div className="text-[10px] opacity-70 truncate">{role.description}</div>
                        </div>
                      </div>
                      <div className="flex gap-2 mt-1 text-[10px] opacity-60">
                        <span>L{role.default_permission_level}</span>
                        <span>{role.default_model}</span>
                        {role.default_max_cost_usd != null && <span>${role.default_max_cost_usd}</span>}
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Template selector (in role mode: filtered by role; in template mode: all) */}
          {(mode === "template" || (mode === "role" && selectedRole)) && (
            <div className="space-y-2">
              <label className="text-xs font-medium text-muted-foreground">
                {mode === "role" ? "Role Templates" : "Workflow Template"}
              </label>
              <div className="grid gap-1.5">
                {filteredTemplates.map((t) => (
                  <button
                    key={t.name}
                    type="button"
                    onClick={() => setSelectedTemplate(t.name)}
                    className={`rounded-md border px-3 py-2 text-left transition-colors ${
                      selectedTemplate === t.name
                        ? "border-primary bg-primary/5"
                        : "border-border hover:border-foreground/30"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <div className="text-sm font-medium">{t.name}</div>
                      {t.role_name && roleBadge(t.role_name)}
                    </div>
                    <div className="text-xs text-muted-foreground">{t.description}</div>
                    <div className="text-[10px] text-muted-foreground/60 mt-1">
                      {t.steps.length} steps
                    </div>
                  </button>
                ))}
              </div>
              {activeTemplate && (
                <div className="bg-muted/50 rounded-md px-3 py-2 space-y-1">
                  <div className="text-xs font-medium text-muted-foreground">Steps:</div>
                  {activeTemplate.steps.map((step, i) => (
                    <div key={i} className="text-xs flex items-center gap-1.5">
                      <span className="text-muted-foreground/60">{i + 1}.</span>
                      <span>{step.title}</span>
                      {step.depends_on_step !== undefined && (
                        <span className="text-[10px] text-muted-foreground">
                          (after step {step.depends_on_step + 1})
                        </span>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          <div>
            <label className="text-xs font-medium text-muted-foreground">Title</label>
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={
                mode === "role" && selectedRole
                  ? `e.g. ${selectedRole === "engine" ? "Optimize factorial sieve" : selectedRole === "frontend" ? "Add search results page" : selectedRole === "ops" ? "Deploy v2.1 to fleet" : "Research Wagstaff primes"}`
                  : mode === "template"
                    ? "e.g. Fix login bug"
                    : "Task title..."
              }
            />
          </div>
          <div>
            <label className="text-xs font-medium text-muted-foreground">Description</label>
            <textarea
              className="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring min-h-[80px] resize-y"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Describe the task..."
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-medium text-muted-foreground">Priority</label>
              <Select value={priority} onValueChange={setPriority}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="low">Low</SelectItem>
                  <SelectItem value="normal">Normal</SelectItem>
                  <SelectItem value="high">High</SelectItem>
                  <SelectItem value="urgent">Urgent</SelectItem>
                </SelectContent>
              </Select>
            </div>
            {mode === "custom" && (
              <div>
                <label className="text-xs font-medium text-muted-foreground">Model</label>
                <Select value={model} onValueChange={setModel}>
                  <SelectTrigger>
                    <SelectValue placeholder="Any" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="opus">Opus</SelectItem>
                    <SelectItem value="sonnet">Sonnet</SelectItem>
                    <SelectItem value="haiku">Haiku</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            )}
          </div>
          <div>
            <label className="text-xs font-medium text-muted-foreground">Permission Level</label>
            <div className="grid grid-cols-4 gap-1.5 mt-1">
              {[
                { level: 0, label: "L0 Read-Only", desc: "Research only" },
                { level: 1, label: "L1 Standard", desc: "Code changes" },
                { level: 2, label: "L2 Trusted", desc: "Full git + subagents" },
                { level: 3, label: "L3 Admin", desc: "Unrestricted" },
              ].map((opt) => (
                <button
                  key={opt.level}
                  type="button"
                  onClick={() => setPermissionLevel(opt.level)}
                  className={`rounded-md border px-2 py-1.5 text-center text-[11px] leading-tight transition-colors ${
                    permissionLevel === opt.level
                      ? opt.level === 3
                        ? "border-red-500 bg-red-500/10 text-red-600"
                        : "border-primary bg-primary/10 text-primary"
                      : "border-border text-muted-foreground hover:border-foreground/30"
                  }`}
                >
                  <div className="font-medium">{opt.label.split(" ")[0]}</div>
                  <div className="text-[10px] opacity-70">{opt.desc}</div>
                </button>
              ))}
            </div>
          </div>
          <div>
            <label className="text-xs font-medium text-muted-foreground">
              Max Cost (USD, optional)
            </label>
            <Input
              type="number"
              value={maxCost}
              onChange={(e) => setMaxCost(e.target.value)}
              placeholder="No limit"
              step="0.01"
              min="0"
              className="h-8"
            />
          </div>
          {/* Cost estimation */}
          <div className="text-xs text-muted-foreground bg-muted/50 rounded-md px-3 py-2 space-y-1">
            <div>
              Estimated: ${costEstimate.low.toFixed(2)} &ndash; ${costEstimate.high.toFixed(2)}
              <span className="ml-1 text-muted-foreground/70">
                ({MODEL_RATES[effectiveModel]?.label || "Sonnet"} rate)
              </span>
            </div>
            {highEstimateWarning && (
              <div className="text-amber-500 flex items-center gap-1">
                <AlertCircle className="size-3" />
                High estimate may exceed remaining daily budget (${remaining?.toFixed(2)})
              </div>
            )}
          </div>
          <Button
            className="w-full"
            onClick={handleSubmit}
            disabled={
              submitting ||
              !title.trim() ||
              (mode === "template" && !selectedTemplate)
            }
          >
            {submitting
              ? "Creating..."
              : (mode === "template" || (mode === "role" && selectedTemplate))
                ? "Expand Template"
                : "Create Task"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// --- Task detail with events ---

function TaskEvents({ taskId }: { taskId: number }) {
  const { events, loading } = useAgentEvents(taskId);

  if (loading) {
    return <div className="text-xs text-muted-foreground py-2">Loading events...</div>;
  }
  if (events.length === 0) {
    return <div className="text-xs text-muted-foreground py-2">No events yet</div>;
  }

  return (
    <div className="space-y-1.5 pt-2 border-t mt-2">
      {events.map((ev) => (
        <div key={ev.id} className="flex items-start gap-2 text-xs">
          <div className="mt-0.5">{eventIcon(ev.event_type)}</div>
          <div className="min-w-0 flex-1">
            <span className="text-foreground">{ev.summary}</span>
            {ev.agent && (
              <span className="ml-1 text-muted-foreground font-mono">{ev.agent}</span>
            )}
            <span className="ml-2 text-muted-foreground">
              {new Date(ev.created_at).toLocaleTimeString()}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

// --- Subtask Tree ---

function SubtaskTree({ children }: { children: AgentTask[] }) {
  if (children.length === 0) return null;

  const completedCount = children.filter(
    (c) => c.status === "completed" || c.status === "cancelled"
  ).length;

  return (
    <div className="mt-2 pt-2 border-t space-y-1">
      <div className="text-xs text-muted-foreground flex items-center gap-1.5">
        <GitBranch className="size-3" />
        <span>
          {completedCount}/{children.length} steps complete
        </span>
      </div>
      {children.map((child, idx) => (
        <div
          key={child.id}
          className="flex items-center gap-2 text-xs pl-4 py-0.5"
        >
          <span className="text-muted-foreground/50 w-4 text-right">{idx + 1}.</span>
          {statusDot(child.status)}
          <span
            className={
              child.status === "completed"
                ? "text-muted-foreground line-through"
                : child.status === "failed"
                  ? "text-red-500"
                  : "text-foreground"
            }
          >
            {child.title.includes(": ") ? child.title.split(": ").slice(1).join(": ") : child.title}
          </span>
          <Badge
            variant={
              child.status === "in_progress"
                ? "default"
                : child.status === "failed"
                  ? "destructive"
                  : "outline"
            }
            className="text-[9px] px-1 py-0 h-4"
          >
            {child.status === "in_progress" ? "Running" : child.status}
          </Badge>
        </div>
      ))}
    </div>
  );
}

function statusDot(status: string) {
  const colors: Record<string, string> = {
    pending: "bg-muted-foreground/30",
    in_progress: "bg-blue-500",
    completed: "bg-green-500",
    failed: "bg-red-500",
    cancelled: "bg-muted-foreground/30",
  };
  return <span className={`size-2 rounded-full shrink-0 ${colors[status] || "bg-muted"}`} />;
}

// --- Task Card ---

function TaskCard({ task, children }: { task: AgentTask; children: AgentTask[] }) {
  const [expanded, setExpanded] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const hasChildren = children.length > 0;

  async function handleCancel() {
    setCancelling(true);
    try {
      await cancelTask(task.id);
      toast.success("Task cancelled");
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to cancel");
    } finally {
      setCancelling(false);
    }
  }

  const canCancel = task.status === "pending" || task.status === "in_progress";

  return (
    <Card className="py-3">
      <CardContent className="p-0 px-4 space-y-1.5">
        <div className="flex items-center justify-between gap-2">
          <button
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-2 min-w-0 text-left"
          >
            {hasChildren ? (
              expanded ? (
                <ChevronDown className="size-4 text-muted-foreground shrink-0" />
              ) : (
                <ChevronRight className="size-4 text-muted-foreground shrink-0" />
              )
            ) : null}
            <span className="text-sm font-medium text-foreground truncate">
              {task.title}
            </span>
            {task.role_name && roleBadge(task.role_name)}
            {task.template_name && (
              <Badge variant="outline" className="text-[10px] px-1.5 py-0 shrink-0">
                {task.template_name}
              </Badge>
            )}
          </button>
          <div className="flex items-center gap-2 flex-shrink-0">
            {statusBadge(task.status)}
            {canCancel && (
              <Button
                variant="outline"
                size="xs"
                className="text-red-600 hover:text-red-700"
                disabled={cancelling}
                onClick={handleCancel}
              >
                {cancelling ? "..." : "Cancel"}
              </Button>
            )}
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
          {priorityBadge(task.priority)}
          {task.agent_model && (
            <Badge variant="outline" className="text-[10px] px-1.5 py-0">
              {task.agent_model}
            </Badge>
          )}
          <Badge
            variant="outline"
            className={`text-[10px] px-1.5 py-0 ${
              task.permission_level === 0
                ? "border-blue-400 text-blue-500"
                : task.permission_level === 3
                  ? "border-red-400 text-red-500"
                  : ""
            }`}
          >
            L{task.permission_level}
          </Badge>
          <span>{formatTime(task.created_at)}</span>
          {task.tokens_used > 0 && (
            <span>{numberWithCommas(task.tokens_used)} tokens</span>
          )}
          {task.cost_usd > 0 && <span>${task.cost_usd.toFixed(4)}</span>}
          {task.max_cost_usd != null && (
            <span className="text-amber-500">max ${task.max_cost_usd.toFixed(2)}</span>
          )}
          <span className="text-muted-foreground/60">#{task.id}</span>
        </div>
        {hasChildren && <SubtaskTree children={children} />}
        {expanded && <TaskEvents taskId={task.id} />}
      </CardContent>
    </Card>
  );
}

// --- Activity Feed ---

function ActivityFeed() {
  const { events, loading } = useAgentEvents();

  if (loading) {
    return (
      <Card className="py-8 border-dashed">
        <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
          Loading activity...
        </CardContent>
      </Card>
    );
  }

  if (events.length === 0) {
    return (
      <Card className="py-8 border-dashed">
        <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
          No agent activity yet.
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-1">
      {events.map((ev: AgentEvent) => (
        <Card key={ev.id} className="py-2">
          <CardContent className="p-0 px-4 flex items-start gap-2">
            <div className="mt-0.5">{eventIcon(ev.event_type)}</div>
            <div className="min-w-0 flex-1 text-xs">
              <div className="flex items-center gap-2">
                <span className="font-medium text-foreground">{ev.summary}</span>
                {ev.task_id && (
                  <span className="text-muted-foreground">task #{ev.task_id}</span>
                )}
              </div>
              <div className="flex items-center gap-2 text-muted-foreground mt-0.5">
                {ev.agent && <span className="font-mono">{ev.agent}</span>}
                <span>{formatTime(ev.created_at)}</span>
              </div>
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

// --- Budget Tab ---

function BudgetCards() {
  const { budgets, loading, refetch } = useAgentBudgets();
  const [editingId, setEditingId] = useState<number | null>(null);
  const [editValue, setEditValue] = useState("");

  if (loading) {
    return (
      <Card className="py-8 border-dashed">
        <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
          Loading budgets...
        </CardContent>
      </Card>
    );
  }

  if (budgets.length === 0) {
    return (
      <Card className="py-8 border-dashed">
        <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
          No budgets configured.
        </CardContent>
      </Card>
    );
  }

  async function handleSave(id: number) {
    const val = parseFloat(editValue);
    if (isNaN(val) || val < 0) {
      toast.error("Invalid budget amount");
      return;
    }
    const { error } = await (await import("@/lib/supabase")).supabase
      .from("agent_budgets")
      .update({ budget_usd: val, updated_at: new Date().toISOString() })
      .eq("id", id);

    if (error) {
      toast.error(error.message);
    } else {
      toast.success("Budget updated");
      refetch();
    }
    setEditingId(null);
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
      {budgets.map((b) => {
        const pct = b.budget_usd > 0 ? Math.min((b.spent_usd / b.budget_usd) * 100, 100) : 0;
        const barColor =
          pct >= 90 ? "bg-red-500" : pct >= 70 ? "bg-amber-500" : "bg-green-500";

        return (
          <Card key={b.id} className="py-4">
            <CardContent className="p-0 px-4 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm font-semibold capitalize">{b.period}</span>
                <button
                  onClick={() => {
                    setEditingId(b.id);
                    setEditValue(b.budget_usd.toString());
                  }}
                  className="text-muted-foreground hover:text-foreground transition-colors"
                >
                  <Pencil className="size-3.5" />
                </button>
              </div>

              {/* Progress bar */}
              <div>
                <div className="flex justify-between text-xs text-muted-foreground mb-1">
                  <span>${b.spent_usd.toFixed(2)} spent</span>
                  <span>${b.budget_usd.toFixed(2)} budget</span>
                </div>
                <div className="h-2 w-full rounded-full bg-muted overflow-hidden">
                  <div
                    className={`h-full rounded-full transition-all ${barColor}`}
                    style={{ width: `${pct}%` }}
                  />
                </div>
              </div>

              <div className="flex justify-between text-xs text-muted-foreground">
                <span>{numberWithCommas(b.tokens_used)} tokens</span>
                <span>since {new Date(b.period_start).toLocaleDateString()}</span>
              </div>

              {editingId === b.id && (
                <div className="flex items-center gap-2 pt-1 border-t">
                  <Input
                    type="number"
                    value={editValue}
                    onChange={(e) => setEditValue(e.target.value)}
                    className="h-7 text-xs"
                    step="0.01"
                    min="0"
                  />
                  <Button size="xs" onClick={() => handleSave(b.id)}>
                    Save
                  </Button>
                  <Button size="xs" variant="outline" onClick={() => setEditingId(null)}>
                    Cancel
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}

// --- Memory Tab ---

function MemoryTab() {
  const { memories, loading, refetch } = useAgentMemory();
  const [adding, setAdding] = useState(false);
  const [newKey, setNewKey] = useState("");
  const [newValue, setNewValue] = useState("");
  const [newCategory, setNewCategory] = useState<MemoryCategory>("general");
  const [editingKey, setEditingKey] = useState<string | null>(null);
  const [editValue, setEditValue] = useState("");

  async function handleAdd() {
    if (!newKey.trim() || !newValue.trim()) return;
    try {
      await upsertMemory(newKey.trim(), newValue.trim(), newCategory);
      toast.success("Memory added");
      setNewKey("");
      setNewValue("");
      setNewCategory("general");
      setAdding(false);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to add memory");
    }
  }

  async function handleUpdate(key: string) {
    if (!editValue.trim()) return;
    try {
      const mem = memories.find((m) => m.key === key);
      await upsertMemory(key, editValue.trim(), mem?.category || "general");
      toast.success("Memory updated");
      setEditingKey(null);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to update");
    }
  }

  async function handleDelete(key: string) {
    try {
      await deleteMemory(key);
      toast.success("Memory deleted");
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete");
    }
  }

  if (loading) {
    return (
      <Card className="py-8 border-dashed">
        <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
          Loading memory...
        </CardContent>
      </Card>
    );
  }

  // Group by category
  const grouped = memories.reduce(
    (acc, mem) => {
      (acc[mem.category] ??= []).push(mem);
      return acc;
    },
    {} as Record<string, AgentMemory[]>
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-xs text-muted-foreground">
          {memories.length} entries &mdash; injected into agent system prompts at spawn time
        </p>
        <Button size="sm" variant="outline" onClick={() => setAdding(!adding)}>
          <Plus className="size-3.5 mr-1" />
          Add Memory
        </Button>
      </div>

      {adding && (
        <Card className="py-3">
          <CardContent className="p-0 px-4 space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <div>
                <label className="text-xs font-medium text-muted-foreground">Key</label>
                <Input
                  value={newKey}
                  onChange={(e) => setNewKey(e.target.value)}
                  placeholder="e.g. proth_test_base_skip"
                  className="h-8"
                />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Category</label>
                <Select value={newCategory} onValueChange={(v) => setNewCategory(v as MemoryCategory)}>
                  <SelectTrigger className="h-8">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {MEMORY_CATEGORIES.map((c) => (
                      <SelectItem key={c} value={c}>
                        {c}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">Value</label>
              <textarea
                className="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring min-h-[60px] resize-y"
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                placeholder="What should agents know?"
              />
            </div>
            <div className="flex gap-2">
              <Button size="sm" onClick={handleAdd} disabled={!newKey.trim() || !newValue.trim()}>
                Save
              </Button>
              <Button size="sm" variant="outline" onClick={() => setAdding(false)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {memories.length === 0 && !adding ? (
        <Card className="py-8 border-dashed">
          <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
            No agent memories yet. Agents will accumulate knowledge as they work.
          </CardContent>
        </Card>
      ) : (
        Object.entries(grouped).map(([category, items]) => (
          <div key={category}>
            <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
              {category}
            </h3>
            <div className="space-y-1.5">
              {items.map((mem) => (
                <Card key={mem.id} className="py-2">
                  <CardContent className="p-0 px-4">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium font-mono text-foreground">
                            {mem.key}
                          </span>
                          {mem.created_by_task && (
                            <span className="text-[10px] text-muted-foreground">
                              task #{mem.created_by_task}
                            </span>
                          )}
                        </div>
                        {editingKey === mem.key ? (
                          <div className="flex items-center gap-2 mt-1">
                            <Input
                              value={editValue}
                              onChange={(e) => setEditValue(e.target.value)}
                              className="h-7 text-xs flex-1"
                            />
                            <Button size="xs" onClick={() => handleUpdate(mem.key)}>
                              Save
                            </Button>
                            <Button
                              size="xs"
                              variant="outline"
                              onClick={() => setEditingKey(null)}
                            >
                              Cancel
                            </Button>
                          </div>
                        ) : (
                          <p className="text-xs text-muted-foreground mt-0.5">{mem.value}</p>
                        )}
                      </div>
                      <div className="flex items-center gap-1 flex-shrink-0">
                        <button
                          onClick={() => {
                            setEditingKey(mem.key);
                            setEditValue(mem.value);
                          }}
                          className="text-muted-foreground hover:text-foreground transition-colors p-1"
                        >
                          <Pencil className="size-3" />
                        </button>
                        <button
                          onClick={() => handleDelete(mem.key)}
                          className="text-muted-foreground hover:text-red-500 transition-colors p-1"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        ))
      )}
    </div>
  );
}

// --- Main Page ---

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
            </TabsList>
          }
        />

        {/* Metric Cards */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4">
          <Card className="py-3">
            <CardContent className="px-4 p-0 flex items-center justify-between">
              <div>
                <div className="text-xs font-medium text-muted-foreground">Active Tasks</div>
                <div className="text-xl font-semibold tabular-nums">{counts.active}</div>
              </div>
              <Bot className="size-4 text-primary" />
            </CardContent>
          </Card>
          <Card className="py-3">
            <CardContent className="px-4 p-0 flex items-center justify-between">
              <div>
                <div className="text-xs font-medium text-muted-foreground">Pending Queue</div>
                <div className="text-xl font-semibold tabular-nums">{counts.pending}</div>
              </div>
              <ListTodo className="size-4 text-amber-500" />
            </CardContent>
          </Card>
          <Card className="py-3">
            <CardContent className="px-4 p-0 flex items-center justify-between">
              <div>
                <div className="text-xs font-medium text-muted-foreground">Completed Today</div>
                <div className="text-xl font-semibold tabular-nums">{counts.completedToday}</div>
              </div>
              <CheckCircle2 className="size-4 text-green-500" />
            </CardContent>
          </Card>
          <Card className="py-3">
            <CardContent className="px-4 p-0 flex items-center justify-between">
              <div>
                <div className="text-xs font-medium text-muted-foreground">Today&apos;s Spend</div>
                <div className="text-xl font-semibold tabular-nums">
                  ${todaySpend.toFixed(2)}
                  {todayLimit > 0 && (
                    <span className="text-xs font-normal text-muted-foreground ml-1">
                      / ${todayLimit.toFixed(2)}
                    </span>
                  )}
                </div>
              </div>
              <DollarSign className="size-4 text-green-600" />
            </CardContent>
          </Card>
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
            <Card className="py-8 border-dashed">
              <CardContent className="p-0 px-4 text-center text-muted-foreground text-sm">
                {filter === "all"
                  ? 'No tasks yet. Click "New Task" to create one.'
                  : `No ${filter === "in_progress" ? "running" : filter} tasks.`}
              </CardContent>
            </Card>
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
      </Tabs>

      <NewTaskDialog open={newTaskOpen} onOpenChange={setNewTaskOpen} />
    </>
  );
}
