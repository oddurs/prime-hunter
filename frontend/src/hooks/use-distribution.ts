"use client";

import { useEffect, useState, useCallback } from "react";
import { supabase } from "@/lib/supabase";

export interface DigitBucket {
  bucket_start: number;
  form: string;
  count: number;
}

export function useDistribution(bucketSize: number = 10) {
  const [distribution, setDistribution] = useState<DigitBucket[]>([]);

  const fetch = useCallback(async () => {
    const { data, error } = await supabase.rpc("get_digit_distribution", {
      bucket_size_param: bucketSize,
    });
    if (!error && data) {
      setDistribution(data as DigitBucket[]);
    }
  }, [bucketSize]);

  useEffect(() => {
    fetch();
    const interval = setInterval(fetch, 10000);
    return () => clearInterval(interval);
  }, [fetch]);

  return { distribution, refetch: fetch };
}
