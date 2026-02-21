"use client";

/**
 * @module use-agent-tasks
 *
 * React hooks and CRUD functions for managing Claude Code agent tasks.
 * Covers task listing with polling, event/timeline feeds,
 * template expansion, role management, log streaming, and tree building
 * for multi-step workflows.
 *
 * @see {@link src/agent.rs} -- Rust-side agent subprocess manager
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface AgentTask {
  id: number;
  title: string;
  description: string;
  status: string;
  priority: string;
  agent_model: string | null;
  assigned_agent: string | null;
  source: string;
  result: Record<string, unknown> | null;
  tokens_used: number;
  cost_usd: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  parent_task_id: number | null;
  max_cost_usd: number | null;
  permission_level: number;
  template_name: string | null;
  on_child_failure: string;
  role_name: string | null;
}

export interface AgentEvent {
  id: number;
  task_id: number | null;
  event_type: string;
  agent: string | null;
  summary: string;
  detail: Record<string, unknown> | null;
  created_at: string;
  tool_name: string | null;
  input_tokens: number | null;
  output_tokens: number | null;
  duration_ms: number | null;
}

export interface AgentLog {
  id: number;
  task_id: number;
  stream: string;
  line_num: number;
  msg_type: string | null;
  content: string;
  created_at: string;
}

export interface AgentTemplate {
  id: number;
  name: string;
  description: string;
  steps: TemplateStep[];
  created_at: string;
  role_name: string | null;
}

export interface TemplateStep {
  title: string;
  description: string;
  permission_level: number;
  depends_on_step?: number;
}

/** A task tree node: a parent task with its children. */
export interface TaskTreeNode {
  task: AgentTask;
  children: AgentTask[];
}

/** An agent role that bundles domain context, permissions, and defaults. */
export interface AgentRole {
  id: number;
  name: string;
  description: string;
  domains: string[];
  default_permission_level: number;
  default_model: string;
  system_prompt: string | null;
  default_max_cost_usd: number | null;
  created_at: string;
  updated_at: string;
}

// --- Task Hooks ---

export function useAgentTasks(statusFilter?: string) {
  const [tasks, setTasks] = useState<AgentTask[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchTasks = useCallback(async () => {
    try {
      const params = new URLSearchParams({ limit: "200" });
      if (statusFilter) params.set("status", statusFilter);
      const resp = await fetch(`${API_BASE}/api/agents/tasks?${params}`);
      if (resp.ok) {
        const body = await resp.json();
        setTasks(body.tasks ?? []);
      }
    } catch {
      /* ignore fetch errors */
    }
    setLoading(false);
  }, [statusFilter]);

  useEffect(() => {
    fetchTasks();
  }, [fetchTasks]);

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(fetchTasks, 5_000);
    return () => clearInterval(interval);
  }, [fetchTasks]);

  return { tasks, loading, refetch: fetchTasks };
}

export function useAgentEvents(taskId?: number) {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchEvents = useCallback(async () => {
    try {
      const params = new URLSearchParams({ limit: "200" });
      if (taskId !== undefined) params.set("task_id", String(taskId));
      const resp = await fetch(`${API_BASE}/api/agents/events?${params}`);
      if (resp.ok) {
        const body = await resp.json();
        setEvents(body.events ?? []);
      }
    } catch {
      /* ignore fetch errors */
    }
    setLoading(false);
  }, [taskId]);

  useEffect(() => {
    fetchEvents();
  }, [fetchEvents]);

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(fetchEvents, 5_000);
    return () => clearInterval(interval);
  }, [fetchEvents]);

  return { events, loading, refetch: fetchEvents };
}

export function useAgentTemplates() {
  const [templates, setTemplates] = useState<AgentTemplate[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchTemplates = useCallback(async () => {
    try {
      const resp = await fetch(`${API_BASE}/api/agents/templates`);
      if (resp.ok) {
        const body = await resp.json();
        setTemplates(body.templates ?? []);
      }
    } catch {
      /* ignore fetch errors */
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(fetchTemplates, 5_000);
    return () => clearInterval(interval);
  }, [fetchTemplates]);

  return { templates, loading, refetch: fetchTemplates };
}

/** Fetch all agent roles with polling. */
export function useAgentRoles() {
  const [roles, setRoles] = useState<AgentRole[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchRoles = useCallback(async () => {
    try {
      const resp = await fetch(`${API_BASE}/api/agents/roles`);
      if (resp.ok) {
        const body = await resp.json();
        setRoles(body.roles ?? []);
      }
    } catch {
      /* ignore fetch errors */
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchRoles();
  }, [fetchRoles]);

  // Poll every 5 seconds
  useEffect(() => {
    const interval = setInterval(fetchRoles, 5_000);
    return () => clearInterval(interval);
  }, [fetchRoles]);

  return { roles, loading, refetch: fetchRoles };
}

// --- Task CRUD ---

export async function createTask(
  title: string,
  description: string,
  priority: string,
  agentModel?: string,
  maxCostUsd?: number,
  permissionLevel: number = 1,
  roleName?: string
) {
  const resp = await fetch(`${API_BASE}/api/agents/tasks`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      title,
      description,
      priority,
      agent_model: agentModel || null,
      source: "manual",
      max_cost_usd: maxCostUsd ?? null,
      permission_level: permissionLevel,
      role_name: roleName ?? null,
    }),
  });
  const body = await resp.json();
  if (!resp.ok) throw new Error(body.error || "Failed to create task");
  return body as AgentTask;
}

/** Expand a template into a parent + child task tree via the REST API. */
export async function expandTemplate(
  name: string,
  title: string,
  description: string,
  priority: string,
  maxCostUsd?: number,
  permissionLevel: number = 1,
  roleName?: string
) {
  const resp = await fetch(`${API_BASE}/api/agents/templates/${encodeURIComponent(name)}/expand`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      title,
      description,
      priority,
      max_cost_usd: maxCostUsd ?? null,
      permission_level: permissionLevel,
      role_name: roleName ?? null,
    }),
  });
  const body = await resp.json();
  if (!resp.ok) throw new Error(body.error || "Failed to expand template");
  return body.parent_task_id as number;
}

/**
 * Build a task tree from a flat list of tasks.
 * Groups tasks by parent_task_id, returning an array of tree nodes.
 * Top-level tasks (no parent) that have children become tree roots.
 * Tasks without children are returned as leaf nodes.
 */
export function buildTaskTree(tasks: AgentTask[]): TaskTreeNode[] {
  const childrenByParent = new Map<number, AgentTask[]>();
  const parentIds = new Set<number>();

  for (const task of tasks) {
    if (task.parent_task_id != null) {
      const siblings = childrenByParent.get(task.parent_task_id) ?? [];
      siblings.push(task);
      childrenByParent.set(task.parent_task_id, siblings);
      parentIds.add(task.parent_task_id);
    }
  }

  const nodes: TaskTreeNode[] = [];
  for (const task of tasks) {
    // Skip child tasks -- they'll appear under their parent
    if (task.parent_task_id != null) continue;

    const children = childrenByParent.get(task.id) ?? [];
    // Sort children by id (creation order)
    children.sort((a, b) => a.id - b.id);
    nodes.push({ task, children });
  }

  return nodes;
}

export async function cancelTask(id: number) {
  const resp = await fetch(`${API_BASE}/api/agents/tasks/${id}/cancel`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to cancel task");
  }
}

// --- Observability Hooks ---

/** Fetch paginated log lines for a task from the REST API. */
export function useAgentLogs(taskId: number | null, stream?: string) {
  const [logs, setLogs] = useState<AgentLog[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [offset, setOffset] = useState(0);
  const limit = 500;

  const fetchLogs = useCallback(async () => {
    if (taskId === null) return;
    setLoading(true);
    const params = new URLSearchParams({ offset: String(offset), limit: String(limit) });
    if (stream) params.set("stream", stream);
    try {
      const resp = await fetch(`${API_BASE}/api/agents/tasks/${taskId}/logs?${params}`);
      const body = await resp.json();
      if (resp.ok) {
        setLogs(body.logs ?? []);
        setTotal(body.total ?? 0);
      }
    } catch { /* ignore */ }
    setLoading(false);
  }, [taskId, stream, offset]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  return { logs, total, loading, offset, setOffset, limit, refetch: fetchLogs };
}

/** Fetch events for a task in chronological order (timeline view). */
export function useAgentTimeline(taskId: number | null) {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchTimeline = useCallback(async () => {
    if (taskId === null) return;
    setLoading(true);
    try {
      const resp = await fetch(`${API_BASE}/api/agents/tasks/${taskId}/timeline`);
      const body = await resp.json();
      if (resp.ok) {
        setEvents(body as AgentEvent[]);
      }
    } catch { /* ignore */ }
    setLoading(false);
  }, [taskId]);

  useEffect(() => {
    fetchTimeline();
  }, [fetchTimeline]);

  return { events, loading, refetch: fetchTimeline };
}
