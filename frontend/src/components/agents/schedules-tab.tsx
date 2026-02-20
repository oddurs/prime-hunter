/**
 * @module agents/schedules-tab
 *
 * Automated task scheduling management. Supports two trigger types:
 * - Cron: time-based scheduling with human-readable descriptions
 * - Event: reactive triggers on PrimeFound, SearchCompleted, etc.
 *
 * Each schedule can create either a single task or expand a template.
 * Schedules are created disabled by default and can be toggled on/off.
 */

import { useState } from "react";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { EmptyState } from "@/components/empty-state";
import {
  useAgentSchedules,
  useAgentRoles,
  useAgentTemplates,
  createSchedule,
  deleteSchedule,
  toggleSchedule,
  type AgentSchedule,
} from "@/hooks/use-agents";
import { Plus, Trash2, CalendarClock, Zap } from "lucide-react";
import { formatTime } from "@/lib/format";
import { roleBadge } from "./helpers";

/** Human-readable cron description for common patterns. */
function cronDescription(expr: string): string {
  const parts = expr.split(/\s+/);
  if (parts.length !== 5) return expr;
  const [min, hour, dom, mon, dow] = parts;
  if (min === "*" && hour === "*" && dom === "*" && mon === "*" && dow === "*") return "Every minute";
  if (dom === "*" && mon === "*" && dow === "*") {
    const h = hour === "*" ? "every hour" : `${hour}:${min.padStart(2, "0")}`;
    return min === "0" && hour !== "*" ? `Daily at ${h} UTC` : `At ${h} UTC`;
  }
  if (dom === "*" && mon === "*" && dow !== "*") {
    const days = ["", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const day = days[parseInt(dow)] || dow;
    return `${day} at ${hour}:${min.padStart(2, "0")} UTC`;
  }
  if (dow === "*" && mon === "*" && dom !== "*") {
    return `Day ${dom} at ${hour}:${min.padStart(2, "0")} UTC`;
  }
  return expr;
}

const EVENT_TYPES = ["PrimeFound", "SearchStarted", "SearchCompleted", "Milestone", "Warning", "Error"] as const;

export function SchedulesTab() {
  const { schedules, loading, refetch } = useAgentSchedules();
  const { roles } = useAgentRoles();
  const { templates } = useAgentTemplates();
  const [creating, setCreating] = useState(false);
  const [submitting, setSubmitting] = useState(false);

  // New schedule form state
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [triggerType, setTriggerType] = useState<"cron" | "event">("cron");
  const [cronExpr, setCronExpr] = useState("0 2 * * *");
  const [eventFilter, setEventFilter] = useState("PrimeFound");
  const [actionType, setActionType] = useState<"task" | "template">("task");
  const [templateName, setTemplateName] = useState("");
  const [roleName, setRoleName] = useState("");
  const [taskTitle, setTaskTitle] = useState("");
  const [taskDescription, setTaskDescription] = useState("");
  const [priority, setPriority] = useState("normal");
  const [maxCost, setMaxCost] = useState("");
  const [permissionLevel, setPermissionLevel] = useState(1);

  function resetForm() {
    setName("");
    setDescription("");
    setTriggerType("cron");
    setCronExpr("0 2 * * *");
    setEventFilter("PrimeFound");
    setActionType("task");
    setTemplateName("");
    setRoleName("");
    setTaskTitle("");
    setTaskDescription("");
    setPriority("normal");
    setMaxCost("");
    setPermissionLevel(1);
  }

  async function handleCreate() {
    if (!name.trim() || !taskTitle.trim()) return;
    setSubmitting(true);
    try {
      await createSchedule({
        name: name.trim(),
        description: description.trim(),
        enabled: false,
        trigger_type: triggerType,
        cron_expr: triggerType === "cron" ? cronExpr : undefined,
        event_filter: triggerType === "event" ? eventFilter : undefined,
        action_type: actionType,
        template_name: actionType === "template" && templateName ? templateName : undefined,
        role_name: roleName || undefined,
        task_title: taskTitle.trim(),
        task_description: taskDescription.trim(),
        priority,
        max_cost_usd: maxCost ? parseFloat(maxCost) : undefined,
        permission_level: permissionLevel,
      });
      toast.success("Schedule created (disabled by default)");
      resetForm();
      setCreating(false);
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to create schedule");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleToggle(s: AgentSchedule) {
    try {
      await toggleSchedule(s.id, !s.enabled);
      toast.success(s.enabled ? "Schedule disabled" : "Schedule enabled");
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to toggle schedule");
    }
  }

  async function handleDelete(s: AgentSchedule) {
    try {
      await deleteSchedule(s.id);
      toast.success("Schedule deleted");
      refetch();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to delete schedule");
    }
  }

  if (loading) {
    return <EmptyState message="Loading schedules..." />;
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <p className="text-xs text-muted-foreground">
          {schedules.length} schedules &mdash; automated task creation via cron or event triggers
        </p>
        <Button size="sm" variant="outline" onClick={() => setCreating(!creating)}>
          <Plus className="size-3.5 mr-1" />
          New Schedule
        </Button>
      </div>

      {creating && (
        <Card className="py-3">
          <CardContent className="p-0 px-4 space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">Name</label>
                <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="my-schedule" className="h-8" />
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Trigger Type</label>
                <Select value={triggerType} onValueChange={(v) => setTriggerType(v as "cron" | "event")}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="cron">Cron (time-based)</SelectItem>
                    <SelectItem value="event">Event (reactive)</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>

            {triggerType === "cron" ? (
              <div>
                <label className="text-xs font-medium text-muted-foreground">Cron Expression</label>
                <Input value={cronExpr} onChange={(e) => setCronExpr(e.target.value)} placeholder="0 2 * * *" className="h-8 font-mono" />
                <p className="text-[10px] text-muted-foreground mt-0.5">minute hour day month weekday (1=Mon..7=Sun)</p>
              </div>
            ) : (
              <div>
                <label className="text-xs font-medium text-muted-foreground">Event Type</label>
                <Select value={eventFilter} onValueChange={setEventFilter}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    {EVENT_TYPES.map((t) => (
                      <SelectItem key={t} value={t}>{t}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">Action</label>
                <Select value={actionType} onValueChange={(v) => setActionType(v as "task" | "template")}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="task">Create Task</SelectItem>
                    <SelectItem value="template">Expand Template</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Role</label>
                <Select value={roleName} onValueChange={setRoleName}>
                  <SelectTrigger className="h-8"><SelectValue placeholder="None" /></SelectTrigger>
                  <SelectContent>
                    {roles.map((r) => (
                      <SelectItem key={r.name} value={r.name}>{r.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>

            {actionType === "template" && (
              <div>
                <label className="text-xs font-medium text-muted-foreground">Template</label>
                <Select value={templateName} onValueChange={setTemplateName}>
                  <SelectTrigger className="h-8"><SelectValue placeholder="Select template..." /></SelectTrigger>
                  <SelectContent>
                    {templates.map((t) => (
                      <SelectItem key={t.name} value={t.name}>{t.name}</SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            )}

            <div>
              <label className="text-xs font-medium text-muted-foreground">Task Title</label>
              <Input value={taskTitle} onChange={(e) => setTaskTitle(e.target.value)} placeholder="What this schedule creates..." className="h-8" />
            </div>
            <div>
              <label className="text-xs font-medium text-muted-foreground">Description</label>
              <textarea
                className="flex w-full rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring min-h-[60px] resize-y"
                value={taskDescription}
                onChange={(e) => setTaskDescription(e.target.value)}
                placeholder="Describe what the created task should do..."
              />
            </div>

            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="text-xs font-medium text-muted-foreground">Priority</label>
                <Select value={priority} onValueChange={setPriority}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="low">Low</SelectItem>
                    <SelectItem value="normal">Normal</SelectItem>
                    <SelectItem value="high">High</SelectItem>
                    <SelectItem value="urgent">Urgent</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Permission</label>
                <Select value={String(permissionLevel)} onValueChange={(v) => setPermissionLevel(parseInt(v))}>
                  <SelectTrigger className="h-8"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="0">L0 Read</SelectItem>
                    <SelectItem value="1">L1 Standard</SelectItem>
                    <SelectItem value="2">L2 Trusted</SelectItem>
                    <SelectItem value="3">L3 Admin</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div>
                <label className="text-xs font-medium text-muted-foreground">Max Cost $</label>
                <Input type="number" value={maxCost} onChange={(e) => setMaxCost(e.target.value)} placeholder="None" className="h-8" step="0.01" min="0" />
              </div>
            </div>

            <div className="flex gap-2">
              <Button size="sm" onClick={handleCreate} disabled={submitting || !name.trim() || !taskTitle.trim()}>
                {submitting ? "Creating..." : "Create Schedule"}
              </Button>
              <Button size="sm" variant="outline" onClick={() => { setCreating(false); resetForm(); }}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {schedules.length === 0 && !creating ? (
        <EmptyState message="No schedules yet. Create one to automate task creation." />
      ) : (
        <div className="space-y-2">
          {schedules.map((s) => (
            <Card key={s.id} className="py-3">
              <CardContent className="p-0 px-4 space-y-1.5">
                <div className="flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 min-w-0">
                    {s.trigger_type === "cron" ? (
                      <CalendarClock className="size-4 text-blue-500 shrink-0" />
                    ) : (
                      <Zap className="size-4 text-amber-500 shrink-0" />
                    )}
                    <span className="text-sm font-medium truncate">{s.name}</span>
                    {s.role_name && roleBadge(s.role_name)}
                    {s.action_type === "template" && s.template_name && (
                      <Badge variant="outline" className="text-[10px] px-1.5 py-0 shrink-0">
                        {s.template_name}
                      </Badge>
                    )}
                  </div>
                  <div className="flex items-center gap-2 flex-shrink-0">
                    <button
                      onClick={() => handleToggle(s)}
                      className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                        s.enabled ? "bg-green-500" : "bg-muted"
                      }`}
                    >
                      <span
                        className={`inline-block size-3.5 transform rounded-full bg-white transition-transform ${
                          s.enabled ? "translate-x-4.5" : "translate-x-0.5"
                        }`}
                      />
                    </button>
                    <button
                      onClick={() => handleDelete(s)}
                      className="text-muted-foreground hover:text-red-500 transition-colors p-1"
                    >
                      <Trash2 className="size-3.5" />
                    </button>
                  </div>
                </div>

                {s.description && (
                  <p className="text-xs text-muted-foreground">{s.description}</p>
                )}

                <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                  {s.trigger_type === "cron" && s.cron_expr && (
                    <span className="font-mono bg-muted px-1.5 py-0.5 rounded text-[11px]">
                      {cronDescription(s.cron_expr)}
                    </span>
                  )}
                  {s.trigger_type === "event" && s.event_filter && (
                    <Badge variant="outline" className="text-[10px] px-1.5 py-0">
                      on {s.event_filter}
                    </Badge>
                  )}
                  <span>→ {s.task_title}</span>
                  {s.fire_count > 0 && (
                    <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                      {s.fire_count}× fired
                    </Badge>
                  )}
                  {s.last_fired_at && (
                    <span>last: {formatTime(s.last_fired_at)}</span>
                  )}
                  {s.max_cost_usd != null && (
                    <span className="text-amber-500">max ${s.max_cost_usd.toFixed(2)}</span>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
