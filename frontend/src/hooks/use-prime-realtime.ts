"use client";

/**
 * @module use-prime-realtime
 *
 * Subscribes to Supabase Realtime `postgres_changes` on the `primes`
 * table. When a new prime is INSERTed by any worker, the hook pushes
 * it into a local state array for toast notifications and live table
 * updates — no polling required.
 *
 * @see {@link src/components/prime-notifier.tsx} — consumes this hook
 */

import { useEffect, useState } from "react";
import { supabase } from "@/lib/supabase";

export interface RealtimePrime {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
}

/** Subscribe to Supabase Realtime INSERT events on the primes table. */
export function usePrimeRealtime() {
  const [newPrime, setNewPrime] = useState<RealtimePrime | null>(null);

  useEffect(() => {
    const channel = supabase
      .channel("primes-inserts")
      .on(
        "postgres_changes",
        { event: "INSERT", schema: "public", table: "primes" },
        (payload) => {
          const row = payload.new as RealtimePrime;
          setNewPrime(row);
        }
      )
      .subscribe();

    return () => {
      supabase.removeChannel(channel);
    };
  }, []);

  return { newPrime };
}
