"use client";

/**
 * @module use-agent-budgets
 *
 * React hooks for agent budget tracking and cost analytics.
 * Provides budget period queries, daily cost breakdowns (by model),
 * template-level cost aggregation, and token anomaly detection.
 *
 * Budget periods (daily/weekly/monthly) are fetched from the REST API
 * `/api/agents/budgets` endpoint; analytics data comes from REST endpoints
 * that aggregate across `agent_tasks` and `agent_events`.
 */

import { useEffect, useState, useCallback } from "react";
import type { AgentTask } from "./use-agent-tasks";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface AgentBudget {
  id: number;
  period: string;
  budget_usd: number;
  spent_usd: number;
  tokens_used: number;
  period_start: string;
  updated_at: string;
}

export interface DailyCostRow {
  date: string;
  model: string;
  total_cost: number;
  total_tokens: number;
  task_count: number;
}

export interface TemplateCostRow {
  template_name: string;
  task_count: number;
  total_cost: number;
  avg_cost: number;
  total_tokens: number;
  avg_tokens: number;
}

export function useAgentBudgets() {
  const [budgets, setBudgets] = useState<AgentBudget[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchBudgets = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/agents/budgets`);
      if (res.ok) {
        const data = await res.json();
        setBudgets(data as AgentBudget[]);
      }
    } catch {
      // Network error — keep previous state
    }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchBudgets();
  }, [fetchBudgets]);

  return { budgets, loading, refetch: fetchBudgets };
}

// --- Analytics Hooks ---

/** Fetch daily cost breakdown for analytics. */
export function useAgentDailyCosts(days: number = 30) {
  const [data, setData] = useState<DailyCostRow[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const resp = await fetch(`${API_BASE}/api/agents/analytics/daily-costs?days=${days}`);
      const body = await resp.json();
      if (resp.ok) setData(body as DailyCostRow[]);
    } catch { /* ignore */ }
    setLoading(false);
  }, [days]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, refetch: fetchData };
}

/** Fetch template-level cost aggregation. */
export function useAgentTemplateCosts() {
  const [data, setData] = useState<TemplateCostRow[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const resp = await fetch(`${API_BASE}/api/agents/analytics/template-costs`);
      const body = await resp.json();
      if (resp.ok) setData(body as TemplateCostRow[]);
    } catch { /* ignore */ }
    setLoading(false);
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, refetch: fetchData };
}

/** Fetch tasks with anomalously high token usage. */
export function useAgentAnomalies(threshold: number = 3) {
  const [data, setData] = useState<AgentTask[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const resp = await fetch(`${API_BASE}/api/agents/analytics/anomalies?threshold=${threshold}`);
      const body = await resp.json();
      if (resp.ok) setData(body as AgentTask[]);
    } catch { /* ignore */ }
    setLoading(false);
  }, [threshold]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return { data, loading, refetch: fetchData };
}
