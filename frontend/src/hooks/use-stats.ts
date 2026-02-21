"use client";

/**
 * @module use-stats
 *
 * React hook that fetches aggregate statistics from the REST API
 * `/api/stats` endpoint: total prime count, per-form breakdown, and the
 * largest known prime (by digit count). Used by the dashboard stat cards.
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface Stats {
  total: number;
  by_form: { form: string; count: number }[];
  largest_digits: number;
  largest_expression: string | null;
}

export function useStats() {
  const [stats, setStats] = useState<Stats | null>(null);

  const fetchStats = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/stats`);
      if (res.ok) {
        const data = await res.json();
        setStats(data as Stats);
      }
    } catch {
      // Network error — keep previous state
    }
  }, []);

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 5000);
    return () => clearInterval(interval);
  }, [fetchStats]);

  return { stats, refetch: fetchStats };
}
