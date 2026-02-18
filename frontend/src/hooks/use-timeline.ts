"use client";

/**
 * @module use-timeline
 *
 * React hook that fetches discovery timeline data from the
 * `get_discovery_timeline` Supabase RPC. Returns time-bucketed counts
 * (day/week/month) grouped by prime form, used by the DiscoveryTimeline
 * area chart on the main dashboard.
 *
 * @see {@link src/components/charts/discovery-timeline.tsx}
 */

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

export interface TimelineBucket {
  bucket: string;
  form: string;
  count: number;
}

export function useTimeline(bucketType: string = "day") {
  const [timeline, setTimeline] = useState<TimelineBucket[]>([]);

  const fetch = useCallback(async () => {
    const { data, error } = await supabase.rpc("get_discovery_timeline", {
      bucket_type: bucketType,
    });
    if (!error && data) {
      setTimeline(data as TimelineBucket[]);
    }
  }, [bucketType]);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 10000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { timeline, refetch: fetch };
}
