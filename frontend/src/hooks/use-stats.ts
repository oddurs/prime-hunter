"use client";

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
