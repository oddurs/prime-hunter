/**
 * @module agents/task-card
 *
 * Card component for displaying an agent task with its subtask tree,
 * expandable event log, and detail dialog. The detail dialog provides
 * timeline, log viewer, and event tabs for deep task inspection.
 */

import { useState } from "react";
import { toast } from "sonner";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  useAgentEvents,
  useAgentTimeline,
  useAgentLogs,
  cancelTask,
  type AgentTask,
  type AgentEvent,
} from "@/hooks/use-agents";
import {
  ChevronDown,
  ChevronRight,
  GitBranch,
  Eye,
  Search,
  Download,
  Stethoscope,
} from "lucide-react";
import { numberWithCommas, formatTime, relativeTime } from "@/lib/format";
import { statusBadge, priorityBadge, eventIcon, statusDot, roleBadge } from "./helpers";

// --- Task Events (expandable detail) ---

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

// --- Task Timeline ---

function TaskTimeline({ taskId }: { taskId: number }) {
  const { events, loading } = useAgentTimeline(taskId);
  const [expandedId, setExpandedId] = useState<number | null>(null);

  if (loading) return <div className="text-xs text-muted-foreground py-4">Loading timeline...</div>;
  if (events.length === 0) return <div className="text-xs text-muted-foreground py-4">No events yet</div>;

  const diagnosis = events.find((e) => e.event_type === "diagnosis");

  return (
    <ScrollArea className="h-[50vh]">
      {diagnosis && (
        <div className="mb-3 p-3 rounded-md bg-amber-50 dark:bg-amber-950/30 border border-amber-200 dark:border-amber-800 text-sm">
          <div className="flex items-center gap-2 font-medium text-amber-700 dark:text-amber-400 mb-1">
            <Stethoscope className="size-4" />
            Failure Diagnosis
          </div>
          <p className="text-amber-600 dark:text-amber-300 text-xs">{diagnosis.summary}</p>
        </div>
      )}
      <div className="space-y-2">
        {events.map((ev) => (
          <div key={ev.id} className="flex items-start gap-2.5 text-xs group">
            <div className="mt-0.5 shrink-0">{eventIcon(ev.event_type)}</div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[9px] px-1.5 py-0 h-4">
                  {ev.event_type}
                </Badge>
                <span className="text-muted-foreground">
                  {relativeTime(ev.created_at)}
                </span>
                {ev.tool_name && (
                  <Badge variant="secondary" className="text-[9px] px-1.5 py-0 h-4">
                    {ev.tool_name}
                  </Badge>
                )}
                {(ev.input_tokens || ev.output_tokens) && (
                  <span className="text-muted-foreground tabular-nums">
                    {numberWithCommas((ev.input_tokens ?? 0) + (ev.output_tokens ?? 0))} tok
                  </span>
                )}
              </div>
              <p className="text-foreground mt-0.5 break-words">{ev.summary}</p>
              {ev.detail && (
                <button
                  className="text-muted-foreground hover:text-foreground text-[10px] mt-0.5 flex items-center gap-1"
                  onClick={() => setExpandedId(expandedId === ev.id ? null : ev.id)}
                >
                  <Eye className="size-3" />
                  {expandedId === ev.id ? "Hide" : "Show"} detail
                </button>
              )}
              {expandedId === ev.id && ev.detail && (
                <pre className="mt-1 p-2 rounded bg-muted text-[10px] font-mono overflow-x-auto max-h-48">
                  {JSON.stringify(ev.detail, null, 2)}
                </pre>
              )}
            </div>
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}

// --- Agent Log Viewer ---

function AgentLogViewer({ taskId }: { taskId: number }) {
  const [stream, setStream] = useState<string | undefined>(undefined);
  const [searchTerm, setSearchTerm] = useState("");
  const { logs, total, loading, offset, setOffset, limit } = useAgentLogs(taskId, stream);

  const filtered = searchTerm
    ? logs.filter((l) => l.content.toLowerCase().includes(searchTerm.toLowerCase()))
    : logs;

  function handleDownload() {
    const text = logs.map((l) => `[${l.stream}:${l.line_num}] ${l.content}`).join("\n");
    const blob = new Blob([text], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `task-${taskId}-logs.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }

  if (loading) return <div className="text-xs text-muted-foreground py-4">Loading logs...</div>;

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <div className="flex gap-1">
          {[
            { label: "All", value: undefined },
            { label: "stdout", value: "stdout" },
            { label: "stderr", value: "stderr" },
          ].map((opt) => (
            <Button
              key={opt.label}
              variant={stream === opt.value ? "default" : "outline"}
              size="xs"
              onClick={() => { setStream(opt.value); setOffset(0); }}
            >
              {opt.label}
            </Button>
          ))}
        </div>
        <div className="relative flex-1">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-muted-foreground" />
          <Input
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Search logs..."
            className="h-7 text-xs pl-7"
          />
        </div>
        <Button variant="outline" size="xs" onClick={handleDownload} title="Download logs">
          <Download className="size-3" />
        </Button>
      </div>

      {filtered.length === 0 ? (
        <div className="text-xs text-muted-foreground py-4 text-center">No log lines recorded</div>
      ) : (
        <ScrollArea className="h-[45vh]">
          <div className="font-mono text-[11px] leading-5 bg-muted/50 rounded p-2">
            {filtered.map((line) => (
              <div key={line.id} className="flex gap-2 hover:bg-muted">
                <span className="text-muted-foreground/50 select-none w-8 text-right shrink-0 tabular-nums">
                  {line.line_num}
                </span>
                <span className={`shrink-0 w-10 ${line.stream === "stderr" ? "text-red-500" : "text-muted-foreground/70"}`}>
                  {line.stream}
                </span>
                <span className="break-all">{line.content}</span>
              </div>
            ))}
          </div>
        </ScrollArea>
      )}

      {total > limit && (
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>
            Showing {offset + 1}-{Math.min(offset + limit, total)} of {total}
          </span>
          <div className="flex gap-1">
            <Button
              variant="outline"
              size="xs"
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - limit))}
            >
              Previous
            </Button>
            <Button
              variant="outline"
              size="xs"
              disabled={offset + limit >= total}
              onClick={() => setOffset(offset + limit)}
            >
              Next
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

// --- Task Detail Dialog ---

function TaskDetailDialog({
  task,
  open,
  onOpenChange,
}: {
  task: AgentTask;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <span className="truncate">{task.title}</span>
            {statusBadge(task.status)}
            <span className="text-xs text-muted-foreground font-normal">#{task.id}</span>
          </DialogTitle>
        </DialogHeader>
        <Tabs defaultValue="timeline" className="flex-1 min-h-0 flex flex-col">
          <TabsList variant="line">
            <TabsTrigger value="timeline">Timeline</TabsTrigger>
            <TabsTrigger value="logs">Logs</TabsTrigger>
            <TabsTrigger value="events">Events</TabsTrigger>
          </TabsList>
          <TabsContent value="timeline" className="flex-1 min-h-0 mt-2">
            <TaskTimeline taskId={task.id} />
          </TabsContent>
          <TabsContent value="logs" className="flex-1 min-h-0 mt-2">
            <AgentLogViewer taskId={task.id} />
          </TabsContent>
          <TabsContent value="events" className="flex-1 min-h-0 mt-2">
            <TaskEvents taskId={task.id} />
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}

// --- Task Card ---

interface TaskCardProps {
  task: AgentTask;
  children: AgentTask[];
}

export function TaskCard({ task, children }: TaskCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [detailOpen, setDetailOpen] = useState(false);
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
            <Button
              variant="outline"
              size="xs"
              onClick={() => setDetailOpen(true)}
            >
              <Eye className="size-3 mr-1" />
              Details
            </Button>
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
      <TaskDetailDialog task={task} open={detailOpen} onOpenChange={setDetailOpen} />
    </Card>
  );
}
