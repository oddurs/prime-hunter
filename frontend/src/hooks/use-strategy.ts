"use client";

/**
 * @module use-strategy
 *
 * Hooks for the AI strategy engine: status, form scores, decision history,
 * and configuration. Fetches from the REST API with 30-second polling.
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

// ── Types ───────────────────────────────────────────────────────

export interface StrategyStatus {
  enabled: boolean;
  tick_interval_secs: number;
  last_tick: string | null;
  monthly_spend_usd: number;
  monthly_budget_usd: number;
  max_concurrent_projects: number;
}

export interface FormScore {
  form: string;
  record_gap: number;
  yield_rate: number;
  cost_efficiency: number;
  coverage_gap: number;
  fleet_fit: number;
  total: number;
}

export interface StrategyDecision {
  id: number;
  decision_type: string;
  form: string | null;
  summary: string;
  reasoning: string;
  params: Record<string, unknown> | null;
  estimated_cost_usd: number | null;
  action_taken: string;
  override_reason: string | null;
  project_id: number | null;
  search_job_id: number | null;
  scores: FormScore[] | null;
  created_at: string;
}

export interface StrategyConfig {
  id: number;
  enabled: boolean;
  max_concurrent_projects: number;
  max_monthly_budget_usd: number;
  max_per_project_budget_usd: number;
  preferred_forms: string[];
  excluded_forms: string[];
  min_idle_workers_to_create: number;
  record_proximity_threshold: number;
  tick_interval_secs: number;
  updated_at: string;
}

// ── Hooks ───────────────────────────────────────────────────────

export function useStrategyStatus() {
  const [status, setStatus] = useState<StrategyStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchStatus = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/strategy/status`);
      if (res.ok) {
        setStatus(await res.json());
        setError(null);
      } else {
        setError("Failed to fetch strategy status");
      }
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStatus();
    const interval = setInterval(fetchStatus, 30_000);
    return () => clearInterval(interval);
  }, [fetchStatus]);

  return { status, loading, error, refetch: fetchStatus };
}

export function useStrategyScores() {
  const [scores, setScores] = useState<FormScore[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchScores = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/strategy/scores`);
      if (res.ok) {
        setScores(await res.json());
        setError(null);
      } else {
        setError("Failed to fetch scores");
      }
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchScores();
    const interval = setInterval(fetchScores, 30_000);
    return () => clearInterval(interval);
  }, [fetchScores]);

  return { scores, loading, error, refetch: fetchScores };
}

export function useStrategyDecisions() {
  const [decisions, setDecisions] = useState<StrategyDecision[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchDecisions = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/strategy/decisions`);
      if (res.ok) {
        setDecisions(await res.json());
        setError(null);
      } else {
        setError("Failed to fetch decisions");
      }
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchDecisions();
    const interval = setInterval(fetchDecisions, 30_000);
    return () => clearInterval(interval);
  }, [fetchDecisions]);

  return { decisions, loading, error, refetch: fetchDecisions };
}

export function useStrategyConfig() {
  const [config, setConfig] = useState<StrategyConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/strategy/config`);
      if (res.ok) {
        setConfig(await res.json());
        setError(null);
      } else {
        setError("Failed to fetch config");
      }
    } catch {
      setError("Network error");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchConfig();
  }, [fetchConfig]);

  return { config, loading, error, refetch: fetchConfig };
}

// ── Actions ─────────────────────────────────────────────────────

export async function updateStrategyConfig(
  updates: Partial<Omit<StrategyConfig, "id" | "updated_at">>
): Promise<StrategyConfig> {
  const resp = await fetch(`${API_BASE}/api/strategy/config`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(updates),
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error(
      (body as Record<string, string>).error || "Failed to update config"
    );
  }
  return resp.json();
}

export async function overrideDecision(
  id: number,
  actionTaken: string,
  reason: string
): Promise<void> {
  const resp = await fetch(
    `${API_BASE}/api/strategy/decisions/${id}/override`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ action_taken: actionTaken, reason }),
    }
  );
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error(
      (body as Record<string, string>).error || "Failed to override"
    );
  }
}

export async function triggerStrategyTick(): Promise<{
  decisions: StrategyDecision[];
  scores: FormScore[];
}> {
  const resp = await fetch(`${API_BASE}/api/strategy/tick`, {
    method: "POST",
  });
  if (!resp.ok) {
    const body = await resp.json().catch(() => ({}));
    throw new Error(
      (body as Record<string, string>).error || "Failed to trigger tick"
    );
  }
  return resp.json();
}
