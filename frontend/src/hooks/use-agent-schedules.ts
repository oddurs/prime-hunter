"use client";

/**
 * @module use-agent-schedules
 *
 * React hooks and CRUD functions for automated agent scheduling.
 * Schedules fire tasks on cron expressions or in response to system
 * events (e.g., prime found, search stalled). Supports enable/disable
 * toggling and full schedule editing.
 *
 * Data source: REST API with polling (every 10 seconds).
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

/** An agent schedule that fires tasks on a cron schedule or in response to events. */
export interface AgentSchedule {
  id: number;
  name: string;
  description: string;
  enabled: boolean;
  trigger_type: string;
  cron_expr: string | null;
  event_filter: string | null;
  action_type: string;
  template_name: string | null;
  role_name: string | null;
  task_title: string;
  task_description: string;
  priority: string;
  max_cost_usd: number | null;
  permission_level: number;
  fire_count: number;
  last_fired_at: string | null;
  last_checked_at: string | null;
  created_at: string;
  updated_at: string;
}

/** Fetch all agent schedules with polling. */
export function useAgentSchedules() {
  const [schedules, setSchedules] = useState<AgentSchedule[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchSchedules = useCallback(async () => {
    try {
      const resp = await fetch(`${API_BASE}/api/schedules`);
      if (resp.ok) {
        const body = await resp.json();
        setSchedules(body.schedules ?? []);
      }
    } catch {
      /* ignore fetch errors */
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchSchedules();
  }, [fetchSchedules]);

  // Poll every 10 seconds
  useEffect(() => {
    const interval = setInterval(fetchSchedules, 10_000);
    return () => clearInterval(interval);
  }, [fetchSchedules]);

  return { schedules, loading, refetch: fetchSchedules };
}

export async function createSchedule(payload: {
  name: string;
  description?: string;
  enabled?: boolean;
  trigger_type: string;
  cron_expr?: string;
  event_filter?: string;
  action_type?: string;
  template_name?: string;
  role_name?: string;
  task_title: string;
  task_description?: string;
  priority?: string;
  max_cost_usd?: number;
  permission_level?: number;
}) {
  const resp = await fetch(`${API_BASE}/api/schedules`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      name: payload.name,
      description: payload.description ?? "",
      enabled: payload.enabled ?? false,
      trigger_type: payload.trigger_type,
      cron_expr: payload.cron_expr ?? null,
      event_filter: payload.event_filter ?? null,
      action_type: payload.action_type ?? "task",
      template_name: payload.template_name ?? null,
      role_name: payload.role_name ?? null,
      task_title: payload.task_title,
      task_description: payload.task_description ?? "",
      priority: payload.priority ?? "normal",
      max_cost_usd: payload.max_cost_usd ?? null,
      permission_level: payload.permission_level ?? 1,
    }),
  });
  const body = await resp.json();
  if (!resp.ok) throw new Error(body.error || "Failed to create schedule");
  return body as AgentSchedule;
}

export async function updateSchedule(
  id: number,
  updates: Partial<Pick<AgentSchedule, "name" | "description" | "enabled" | "trigger_type" | "cron_expr" | "event_filter" | "action_type" | "template_name" | "role_name" | "task_title" | "task_description" | "priority" | "max_cost_usd" | "permission_level">>
) {
  const resp = await fetch(`${API_BASE}/api/schedules/${id}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(updates),
  });
  const body = await resp.json();
  if (!resp.ok) throw new Error(body.error || "Failed to update schedule");
  return body as AgentSchedule;
}

export async function deleteSchedule(id: number) {
  const resp = await fetch(`${API_BASE}/api/schedules/${id}`, {
    method: "DELETE",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error((body as Record<string, string>).error || "Failed to delete schedule");
  }
}

export async function toggleSchedule(id: number, enabled: boolean) {
  const resp = await fetch(`${API_BASE}/api/schedules/${id}/toggle`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ enabled }),
  });
  const body = await resp.json();
  if (!resp.ok) throw new Error(body.error || "Failed to toggle schedule");
  return body as AgentSchedule;
}
