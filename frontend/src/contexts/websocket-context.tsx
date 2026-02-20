"use client";

/**
 * @module websocket-context
 *
 * React context that provides coordination data to any component via `useWs()`.
 * Only one connection/polling loop is created at the `<WebSocketProvider>` level
 * in the root layout, and all child components share the same data.
 *
 * Transport selection:
 * - `NEXT_PUBLIC_USE_POLLING=true` → REST polling via `usePolling()` (Vercel deploy)
 * - Otherwise → WebSocket via `useWebSocket()` (same-origin / local dev)
 *
 * @see {@link src/hooks/use-websocket.ts} — WebSocket transport
 * @see {@link src/hooks/use-polling.ts} — REST polling transport
 */

import { createContext, useContext, type ReactNode } from "react";
import { useWebSocket, type WsData } from "@/hooks/use-websocket";
import { usePolling } from "@/hooks/use-polling";

const WebSocketContext = createContext<WsData | null>(null);

/** Provider that uses WebSocket transport (same-origin / local dev). */
function WsProvider({ children }: { children: ReactNode }) {
  const data = useWebSocket();
  return (
    <WebSocketContext.Provider value={data}>{children}</WebSocketContext.Provider>
  );
}

/** Provider that uses REST polling transport (Vercel deploy). */
function PollingProvider({ children }: { children: ReactNode }) {
  const data = usePolling();
  return (
    <WebSocketContext.Provider value={data}>{children}</WebSocketContext.Provider>
  );
}

/** Picks the right provider based on build-time env var. */
export const WebSocketProvider =
  process.env.NEXT_PUBLIC_USE_POLLING === "true" ? PollingProvider : WsProvider;

export function useWs(): WsData {
  const ctx = useContext(WebSocketContext);
  if (!ctx) {
    throw new Error("useWs must be used within a WebSocketProvider");
  }
  return ctx;
}
