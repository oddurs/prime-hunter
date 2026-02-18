"use client";

/**
 * @module use-form-leaderboard
 *
 * React hook that fetches per-form aggregate statistics from the
 * `get_form_leaderboard` Supabase RPC. Returns an array of form entries
 * sorted by prime count descending, each with: count, largest digit count,
 * largest expression, latest discovery time, and verified percentage.
 *
 * Polls every 10 seconds to stay reasonably current without excess load.
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

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

  const fetch = useCallback(async () => {
    const { data, error } = await supabase.rpc("get_form_leaderboard");
    if (!error && data) {
      setEntries(data as FormLeaderboardEntry[]);
    }
  }, []);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 10000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { entries, refetch: fetch };
}
