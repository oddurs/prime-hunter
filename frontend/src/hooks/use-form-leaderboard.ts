"use client";

/**
 * @module use-form-leaderboard
 *
 * React hook that fetches per-form aggregate statistics from the REST API
 * `/api/stats/leaderboard` endpoint. Returns an array of form entries
 * sorted by prime count descending, each with: count, largest digit count,
 * largest expression, latest discovery time, and verified percentage.
 *
 * Polls every 10 seconds to stay reasonably current without excess load.
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface FormLeaderboardEntry {
  form: string;
  count: number;
  largest_digits: number;
  largest_expression: string;
  latest_found_at: string;
  verified_count: number;
  verified_pct: number;
}

export function useFormLeaderboard() {
  const [entries, setEntries] = useState<FormLeaderboardEntry[]>([]);

  const fetchLeaderboard = useCallback(async () => {
    try {
      const res = await fetch(`${API_BASE}/api/stats/leaderboard`);
      if (res.ok) {
        const data = await res.json();
        setEntries(data as FormLeaderboardEntry[]);
      }
    } catch {
      // Network error — keep previous state
    }
  }, []);

  useEffect(() => {
    fetchLeaderboard();
    const interval = setInterval(fetchLeaderboard, 10000);
    return () => clearInterval(interval);
  }, [fetchLeaderboard]);

  return { entries, refetch: fetchLeaderboard };
}
