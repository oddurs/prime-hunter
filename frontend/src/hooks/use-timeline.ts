"use client";

/**
 * @module use-timeline
 *
 * React hook that fetches discovery timeline data from the REST API
 * `/api/stats/timeline` endpoint. Returns time-bucketed counts
 * (day/week/month) grouped by prime form, used by the DiscoveryTimeline
 * area chart on the main dashboard.
 *
 * @see {@link src/components/charts/discovery-timeline.tsx}
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface TimelineBucket {
  bucket: string;
  form: string;
  count: number;
}

export function useTimeline(bucketType: string = "day") {
  const [timeline, setTimeline] = useState<TimelineBucket[]>([]);

  const fetchTimeline = useCallback(async () => {
    try {
      const res = await fetch(
        `${API_BASE}/api/stats/timeline?bucket_type=${bucketType}`
      );
      if (res.ok) {
        const data = await res.json();
        setTimeline(data as TimelineBucket[]);
      }
    } catch {
      // Network error — keep previous state
    }
  }, [bucketType]);

  useEffect(() => {
    fetchTimeline();
    const interval = setInterval(fetchTimeline, 10000);
    return () => clearInterval(interval);
  }, [fetchTimeline]);

  return { timeline, refetch: fetchTimeline };
}
