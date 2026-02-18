"use client";

/**
 * @module websocket-context
 *
 * React context that wraps `useWebSocket()` and makes coordination data
 * available to any component via `useWs()`. This avoids multiple WebSocket
 * connections — only one is created at the `<WebSocketProvider>` level in
 * the root layout, and all child components share the same connection.
 *
 * @see {@link src/hooks/use-websocket.ts} — the underlying WebSocket hook
 */

import { createContext, useContext, type ReactNode } from "react";
import { useWebSocket, type WsData } from "@/hooks/use-websocket";

const WebSocketContext = createContext<WsData | null>(null);

export function WebSocketProvider({ children }: { children: ReactNode }) {
  const ws = useWebSocket();
  return (
    <WebSocketContext.Provider value={ws}>{children}</WebSocketContext.Provider>
  );
}

export function useWs(): WsData {
  const ctx = useContext(WebSocketContext);
  if (!ctx) {
    throw new Error("useWs must be used within a WebSocketProvider");
  }
  return ctx;
}
