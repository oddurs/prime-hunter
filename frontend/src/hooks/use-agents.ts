"use client";

/**
 * @module use-agents
 *
 * React hooks for managing Claude Code agent tasks via Supabase.
 *
 * Provides CRUD operations on the `agent_tasks` table: create tasks,
 * poll for status updates, cancel running agents, expand templates,
 * and build task trees for multi-step workflows. Also provides role
 * management for domain-specific agent configurations.
 *
 * @see {@link src/agent.rs} — Rust-side agent subprocess manager
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

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
}

export interface AgentBudget {
  id: number;
  period: string;
  budget_usd: number;
  spent_usd: number;
  tokens_used: number;
  period_start: string;
  updated_at: string;
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

export function useAgentTasks(statusFilter?: string) {
  const [tasks, setTasks] = useState<AgentTask[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchTasks = useCallback(async () => {
    let query = supabase
      .from("agent_tasks")
      .select("*")
      .order("created_at", { ascending: false })
      .limit(200);

    if (statusFilter) {
      query = query.eq("status", statusFilter);
    }

    const { data, error } = await query;
    if (!error && data) {
      setTasks(data as AgentTask[]);
    }
    setLoading(false);
  }, [statusFilter]);

  useEffect(() => {
    fetchTasks();
  }, [fetchTasks]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_tasks_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_tasks" },
        () => {
          fetchTasks();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchTasks]);

  return { tasks, loading, refetch: fetchTasks };
}

export function useAgentEvents(taskId?: number) {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchEvents = useCallback(async () => {
    let query = supabase
      .from("agent_events")
      .select("*")
      .order("created_at", { ascending: false })
      .limit(200);

    if (taskId !== undefined) {
      query = query.eq("task_id", taskId);
    }

    const { data, error } = await query;
    if (!error && data) {
      setEvents(data as AgentEvent[]);
    }
    setLoading(false);
  }, [taskId]);

  useEffect(() => {
    fetchEvents();
  }, [fetchEvents]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel(`agent_events_changes_${taskId ?? "all"}`)
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "agent_events" },
        () => {
          fetchEvents();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchEvents, taskId]);

  return { events, loading, refetch: fetchEvents };
}

export function useAgentBudgets() {
  const [budgets, setBudgets] = useState<AgentBudget[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchBudgets = useCallback(async () => {
    const { data, error } = await supabase
      .from("agent_budgets")
      .select("*")
      .order("id");

    if (!error && data) {
      setBudgets(data as AgentBudget[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchBudgets();
  }, [fetchBudgets]);

  return { budgets, loading, refetch: fetchBudgets };
}

export function useAgentTemplates() {
  const [templates, setTemplates] = useState<AgentTemplate[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchTemplates = useCallback(async () => {
    const { data, error } = await supabase
      .from("agent_templates")
      .select("*")
      .order("name");

    if (!error && data) {
      setTemplates(data as AgentTemplate[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchTemplates();
  }, [fetchTemplates]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_templates_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_templates" },
        () => {
          fetchTemplates();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchTemplates]);

  return { templates, loading, refetch: fetchTemplates };
}

/** Fetch all agent roles with Supabase realtime subscription. */
export function useAgentRoles() {
  const [roles, setRoles] = useState<AgentRole[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchRoles = useCallback(async () => {
    const { data, error } = await supabase
      .from("agent_roles")
      .select("*")
      .order("name");

    if (!error && data) {
      setRoles(data as AgentRole[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchRoles();
  }, [fetchRoles]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_roles_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_roles" },
        () => {
          fetchRoles();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchRoles]);

  return { roles, loading, refetch: fetchRoles };
}

export async function createTask(
  title: string,
  description: string,
  priority: string,
  agentModel?: string,
  maxCostUsd?: number,
  permissionLevel: number = 1,
  roleName?: string
) {
  const { data, error } = await supabase
    .from("agent_tasks")
    .insert({
      title,
      description,
      priority,
      agent_model: agentModel || null,
      source: "manual",
      max_cost_usd: maxCostUsd ?? null,
      permission_level: permissionLevel,
      role_name: roleName ?? null,
    })
    .select()
    .single();

  if (error) throw error;
  return data as AgentTask;
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
  const resp = await fetch(`/api/agents/templates/${encodeURIComponent(name)}/expand`, {
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
    // Skip child tasks — they'll appear under their parent
    if (task.parent_task_id != null) continue;

    const children = childrenByParent.get(task.id) ?? [];
    // Sort children by id (creation order)
    children.sort((a, b) => a.id - b.id);
    nodes.push({ task, children });
  }

  return nodes;
}

// --- Agent Memory ---

export interface AgentMemory {
  id: number;
  key: string;
  value: string;
  category: string;
  created_by_task: number | null;
  created_at: string;
  updated_at: string;
}

const MEMORY_CATEGORIES = [
  "pattern",
  "convention",
  "gotcha",
  "preference",
  "architecture",
  "general",
] as const;

export type MemoryCategory = (typeof MEMORY_CATEGORIES)[number];
export { MEMORY_CATEGORIES };

export function useAgentMemory() {
  const [memories, setMemories] = useState<AgentMemory[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchMemories = useCallback(async () => {
    const { data, error } = await supabase
      .from("agent_memory")
      .select("*")
      .order("category")
      .order("key");

    if (!error && data) {
      setMemories(data as AgentMemory[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchMemories();
  }, [fetchMemories]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_memory_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_memory" },
        () => {
          fetchMemories();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, [fetchMemories]);

  return { memories, loading, refetch: fetchMemories };
}

export async function upsertMemory(
  key: string,
  value: string,
  category: string = "general"
) {
  const { data, error } = await supabase
    .from("agent_memory")
    .upsert({ key, value, category, updated_at: new Date().toISOString() }, { onConflict: "key" })
    .select()
    .single();

  if (error) throw error;
  return data as AgentMemory;
}

export async function deleteMemory(key: string) {
  const { error } = await supabase.from("agent_memory").delete().eq("key", key);
  if (error) throw error;
}

export async function cancelTask(id: number) {
  const { error } = await supabase
    .from("agent_tasks")
    .update({ status: "cancelled", completed_at: new Date().toISOString() })
    .eq("id", id)
    .in("status", ["pending", "in_progress"]);

  if (error) throw error;
}
