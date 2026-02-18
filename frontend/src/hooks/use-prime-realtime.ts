"use client";

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
