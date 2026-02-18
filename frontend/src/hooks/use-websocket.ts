"use client";

import { useEffect, useRef, useState, useCallback } from "react";

export interface Status {
  active: boolean;
  checkpoint: {
    type: string;
    last_n?: number;
    digit_count?: number;
    half_value?: string;
    start?: number;
    end?: number;
    min_digits?: number;
    max_digits?: number;
    min_n?: number;
    max_n?: number;
  } | null;
}

export interface HardwareMetrics {
  cpu_usage_percent: number;
  memory_used_gb: number;
  memory_total_gb: number;
  memory_usage_percent: number;
  disk_used_gb: number;
  disk_total_gb: number;
  disk_usage_percent: number;
  load_avg_1m: number;
  load_avg_5m: number;
  load_avg_15m: number;
}

export interface WorkerStatus {
  worker_id: string;
  hostname: string;
  cores: number;
  search_type: string;
  search_params: string;
  current: string;
  tested: number;
  found: number;
  uptime_secs: number;
  last_heartbeat_secs_ago: number;
  checkpoint?: string;
  metrics?: HardwareMetrics;
}

export interface Deployment {
  id: number;
  hostname: string;
  ssh_user: string;
  search_type: string;
  search_params: string;
  worker_id: string;
  status: string;
  error: string | null;
  remote_pid: number | null;
  started_at: string;
}

export interface FleetData {
  workers: WorkerStatus[];
  total_workers: number;
  total_cores: number;
  total_tested: number;
  total_found: number;
}

export interface ManagedSearch {
  id: number;
  search_type: string;
  params: {
    search_type: string;
    start?: number;
    end?: number;
    base?: number;
    min_digits?: number;
    max_digits?: number;
    k?: number;
    min_n?: number;
    max_n?: number;
  };
  status: "running" | "paused" | "completed" | "cancelled" | { failed: { reason: string } };
  started_at: string;
  stopped_at: string | null;
  pid: number | null;
  worker_id: string;
  tested: number;
  found: number;
}

export interface Notification {
  id: number;
  kind: string;
  title: string;
  details: string[];
  count: number;
  timestamp_ms: number;
}

/** Coordination-only WebSocket data. Prime data comes from Supabase. */
export interface WsData {
  status: Status | null;
  fleet: FleetData | null;
  coordinator: HardwareMetrics | null;
  searches: ManagedSearch[];
  deployments: Deployment[];
  notifications: Notification[];
  connected: boolean;
  sendMessage: (msg: object) => void;
}

export function useWebSocket(): WsData {
  const [status, setStatus] = useState<Status | null>(null);
  const [fleet, setFleet] = useState<FleetData | null>(null);
  const [searches, setSearches] = useState<ManagedSearch[]>([]);
  const [deployments, setDeployments] = useState<Deployment[]>([]);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [coordinator, setCoordinator] = useState<HardwareMetrics | null>(null);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const connectRef = useRef<() => void>(() => {});

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl =
      process.env.NEXT_PUBLIC_WS_URL ||
      `${protocol}//${window.location.host}/ws`;

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);

    ws.onclose = () => {
      setConnected(false);
      wsRef.current = null;
      reconnectTimerRef.current = setTimeout(() => {
        connectRef.current();
      }, 3000);
    };

    ws.onerror = () => ws.close();

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.type === "update") {
          setStatus(data.status);
          if (data.fleet) setFleet(data.fleet);
          if (data.searches) setSearches(data.searches);
          if (data.deployments) setDeployments(data.deployments);
          if (data.notifications) setNotifications(data.notifications);
          setCoordinator(data.coordinator ?? null);
        } else if (data.type === "notification") {
          const notif = data.notification as Notification;
          setNotifications((prev) => {
            const next = [notif, ...prev.filter((n) => n.id !== notif.id)];
            return next.slice(0, 50);
          });
        }
      } catch {
        // ignore malformed messages
      }
    };
  }, []);

  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  useEffect(() => {
    connect();
    return () => {
      clearTimeout(reconnectTimerRef.current);
      wsRef.current?.close();
    };
  }, [connect]);

  const sendMessage = useCallback((msg: object) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  return {
    status,
    fleet,
    coordinator,
    searches,
    deployments,
    notifications,
    connected,
    sendMessage,
  };
}
