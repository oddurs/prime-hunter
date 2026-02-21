"use client";

/**
 * @module use-prime-realtime
 *
 * Listens for real-time `prime_found` messages from the WebSocket
 * connection. When the backend discovers a new prime, it pushes
 * a `{ type: "prime_found", prime: { ... } }` message over the
 * WebSocket. This hook surfaces the latest such event for toast
 * notifications and live table updates.
 *
 * @see {@link src/components/prime-notifier.tsx} — consumes this hook
 * @see {@link src/hooks/use-websocket.ts} — handles the raw WS message
 */

import { useEffect, useRef } from "react";
import { useWs } from "@/contexts/websocket-context";

export interface RealtimePrime {
  id: number;
  form: string;
  expression: string;
  digits: number;
  found_at: string;
}

/** Subscribe to real-time prime discovery events via WebSocket. */
export function usePrimeRealtime(onPrimeFound?: (prime: RealtimePrime) => void) {
  const { lastPrimeFound } = useWs();
  const callbackRef = useRef(onPrimeFound);
  callbackRef.current = onPrimeFound;

  useEffect(() => {
    if (lastPrimeFound && callbackRef.current) {
      callbackRef.current({
        id: 0, // id not available from WS event; consumers should refetch if needed
        form: lastPrimeFound.form,
        expression: lastPrimeFound.expression,
        digits: lastPrimeFound.digits,
        found_at: new Date(lastPrimeFound.timestamp_ms).toISOString(),
      });
    }
  }, [lastPrimeFound]);

  return { newPrime: lastPrimeFound ? {
    id: 0,
    form: lastPrimeFound.form,
    expression: lastPrimeFound.expression,
    digits: lastPrimeFound.digits,
    found_at: new Date(lastPrimeFound.timestamp_ms).toISOString(),
  } as RealtimePrime : null };
}
