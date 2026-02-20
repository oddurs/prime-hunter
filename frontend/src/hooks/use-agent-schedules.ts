"use client";

/**
 * @module use-agent-schedules
 *
 * React hooks and CRUD functions for automated agent scheduling.
 * Schedules fire tasks on cron expressions or in response to system
 * events (e.g., prime found, search stalled). Supports enable/disable
 * toggling and full schedule editing.
 *
 * Data source: `agent_schedules` table with Supabase realtime.
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

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

/** Fetch all agent schedules with Supabase realtime subscription. */
export function useAgentSchedules() {
  const [schedules, setSchedules] = useState<AgentSchedule[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchSchedules = useCallback(async () => {
    const { data, error } = await supabase
      .from("agent_schedules")
      .select("*")
      .order("name");

    if (!error && data) {
      setSchedules(data as AgentSchedule[]);
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchSchedules();
  }, [fetchSchedules]);

  // Realtime subscription
  useEffect(() => {
    const channel = supabase
      .channel("agent_schedules_changes")
      .on(
        "postgres_changes",
        { event: "*", schema: "public", table: "agent_schedules" },
        () => {
          fetchSchedules();
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
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
  const { data, error } = await supabase
    .from("agent_schedules")
    .insert({
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
    })
    .select()
    .single();

  if (error) throw error;
  return data as AgentSchedule;
}

export async function updateSchedule(
  id: number,
  updates: Partial<Pick<AgentSchedule, "name" | "description" | "enabled" | "trigger_type" | "cron_expr" | "event_filter" | "action_type" | "template_name" | "role_name" | "task_title" | "task_description" | "priority" | "max_cost_usd" | "permission_level">>
) {
  const { data, error } = await supabase
    .from("agent_schedules")
    .update({ ...updates, updated_at: new Date().toISOString() })
    .eq("id", id)
    .select()
    .single();

  if (error) throw error;
  return data as AgentSchedule;
}

export async function deleteSchedule(id: number) {
  const { error } = await supabase.from("agent_schedules").delete().eq("id", id);
  if (error) throw error;
}

export async function toggleSchedule(id: number, enabled: boolean) {
  return updateSchedule(id, { enabled });
}
