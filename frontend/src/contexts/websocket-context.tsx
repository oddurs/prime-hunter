"use client";

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
