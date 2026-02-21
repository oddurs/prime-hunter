"use client";

/**
 * @module use-distribution
 *
 * React hook that fetches digit-count distribution data from the REST API
 * `/api/stats/distribution` endpoint. Returns bucketed counts
 * grouped by prime form, used by the DigitDistribution bar chart.
 *
 * @see {@link src/components/charts/digit-distribution.tsx}
 */

import { useEffect, useState, useCallback } from "react";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "";

export interface DigitBucket {
  bucket_start: number;
  form: string;
  count: number;
}

export function useDistribution(bucketSize: number = 10) {
  const [distribution, setDistribution] = useState<DigitBucket[]>([]);

  const fetchDistribution = useCallback(async () => {
    try {
      const res = await fetch(
        `${API_BASE}/api/stats/distribution?bucket_size=${bucketSize}`
      );
      if (res.ok) {
        const data = await res.json();
        setDistribution(data as DigitBucket[]);
      }
    } catch {
      // Network error — keep previous state
    }
  }, [bucketSize]);

  useEffect(() => {
    fetchDistribution();
    const interval = setInterval(fetchDistribution, 10000);
    return () => clearInterval(interval);
  }, [fetchDistribution]);

  return { distribution, refetch: fetchDistribution };
}
