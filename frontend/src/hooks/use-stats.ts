"use client";

/**
 * @module use-stats
 *
 * React hook that fetches aggregate statistics from the `get_stats`
 * Supabase RPC: total prime count, per-form breakdown, and the largest
 * known prime (by digit count). Used by the dashboard stat cards.
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

export interface Stats {
  total: number;
  by_form: { form: string; count: number }[];
  largest_digits: number;
  largest_expression: string | null;
}

export function useStats() {
  const [stats, setStats] = useState<Stats | null>(null);

  const fetch = useCallback(async () => {
    const { data, error } = await supabase.rpc("get_stats");
    if (!error && data) {
      setStats(data as Stats);
    }
  }, []);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 5000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { stats, refetch: fetch };
}
