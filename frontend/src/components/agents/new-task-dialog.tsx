/**
 * @module agents/new-task-dialog
 *
 * Dialog for creating new agent tasks. Supports three modes:
 * - Role: select a domain role (engine, frontend, ops, research) with presets
 * - Template: expand a multi-step workflow template
 * - Custom: create a single task with full control
 *
 * Shows cost estimates based on model rates and description length,
 * with warnings when estimates approach the daily budget limit.
 */

import { useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
  useAgentBudgets,
  useAgentTemplates,
  useAgentRoles,
  createTask,
  expandTemplate,
} from "@/hooks/use-agents";
import {
  Bot,
  Cog,
  LayoutTemplate,
  AlertCircle,
} from "lucide-react";
import { ROLE_CONFIG, roleBadge, MODEL_RATES, estimateCostRange } from "./helpers";

interface NewTaskDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NewTaskDialog({ open, onOpenChange }: NewTaskDialogProps) {
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
