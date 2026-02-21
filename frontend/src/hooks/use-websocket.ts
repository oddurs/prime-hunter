"use client";

/**
 * @module use-websocket
 *
 * WebSocket client hook for real-time coordination data from the Rust
 * backend (`dashboard.rs`). Connects to `/ws` and receives JSON frames
 * every 2 seconds containing:
 *
 * - **Fleet data**: worker list with heartbeat status, hardware metrics
 * - **Search data**: active/completed search processes
 * - **Deployment data**: remote worker deployments via SSH
 * - **Status data**: coordinator uptime, active search checkpoint
 *
 * Handles reconnection with exponential backoff. This is the only
 * WebSocket in the app — all Supabase data uses REST/Realtime instead.
 *
 * @see {@link src/contexts/websocket-context.tsx} — provides this via React context
 */

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

export type NodeStatus = WorkerStatus;

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

export interface ServerInfo {
  hostname: string;
  role: "service" | "compute";
  metrics: HardwareMetrics | null;
  worker_count: number;
  cores: number;
  worker_ids: string[];
  total_tested: number;
  total_found: number;
  uptime_secs: number;
}

export interface FleetData {
  workers: WorkerStatus[];
  servers?: ServerInfo[];
  total_workers: number;
  total_cores: number;
  total_tested: number;
  total_found: number;
}

export type NetworkData = FleetData;

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

export interface AgentTaskSummary {
  id: number;
  title: string;
  status: string;
  priority: string;
  agent_model: string | null;
  tokens_used: number;
  cost_usd: number;
  created_at: string;
}

export interface AgentBudgetSummary {
  id: number;
  period: string;
  budget_usd: number;
  spent_usd: number;
  tokens_used: number;
}

export interface AgentInfo {
  task_id: number;
  title: string;
  model: string;
  status: string;
  started_at: string;
  pid: number | null;
}

/** A PostgreSQL-backed search job with block-based work distribution.
 * Created via `POST /api/search_jobs` or by the projects system.
 * Workers claim blocks from these jobs via `claim_work_block()`. */
export interface SearchJob {
  id: number;
  search_type: string;
  params: Record<string, unknown>;
  status: "pending" | "running" | "paused" | "completed" | "cancelled" | "failed";
  error: string | null;
  created_at: string;
  started_at: string | null;
  stopped_at: string | null;
  range_start: number;
  range_end: number;
  block_size: number;
  total_tested: number;
  total_found: number;
}

export interface ProjectSummary {
  slug: string;
  name: string;
  form: string;
  objective: string;
  status: string;
  total_tested: number;
  total_found: number;
  best_digits: number;
  total_cost_usd: number;
}

export interface RecordSummary {
  form: string;
  expression: string;
  digits: number;
  holder: string | null;
  our_best_digits: number;
}

/** Coordination-only WebSocket data. Prime data comes from Supabase. */
export interface WsData {
  status: Status | null;
  fleet: FleetData | null;
  coordinator: HardwareMetrics | null;
  searches: ManagedSearch[];
  searchJobs: SearchJob[];
  deployments: Deployment[];
  notifications: Notification[];
  agentTasks: AgentTaskSummary[];
  agentBudgets: AgentBudgetSummary[];
  runningAgents: AgentInfo[];
  projects: ProjectSummary[];
  records: RecordSummary[];
  connected: boolean;
  sendMessage: (msg: object) => void;
}

export function useWebSocket(): WsData {
  const [status, setStatus] = useState<Status | null>(null);
  const [fleet, setFleet] = useState<FleetData | null>(null);
  const [searches, setSearches] = useState<ManagedSearch[]>([]);
  const [searchJobs, setSearchJobs] = useState<SearchJob[]>([]);
  const [deployments, setDeployments] = useState<Deployment[]>([]);
  const [notifications, setNotifications] = useState<Notification[]>([]);
  const [coordinator, setCoordinator] = useState<HardwareMetrics | null>(null);
  const [agentTasks, setAgentTasks] = useState<AgentTaskSummary[]>([]);
  const [agentBudgets, setAgentBudgets] = useState<AgentBudgetSummary[]>([]);
  const [runningAgents, setRunningAgents] = useState<AgentInfo[]>([]);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [records, setRecords] = useState<RecordSummary[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const reconnectDelayRef = useRef(1000);
  const connectRef = useRef<() => void>(() => {});

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl =
      process.env.NEXT_PUBLIC_WS_URL ||
      `${protocol}//${window.location.host}/ws`;

    const ws = new WebSocket(wsUrl);
    wsRef.current = ws;

    ws.onopen = () => {
      setConnected(true);
      reconnectDelayRef.current = 1000; // reset on success
    };

    ws.onclose = () => {
      setConnected(false);
      wsRef.current = null;
      const delay = reconnectDelayRef.current;
      // Exponential backoff with jitter: 1s → 2s → 4s → ... → 30s max
      const jitter = Math.random() * 500;
      reconnectTimerRef.current = setTimeout(() => {
        connectRef.current();
      }, delay + jitter);
      reconnectDelayRef.current = Math.min(delay * 2, 30000);
    };

    ws.onerror = () => ws.close();

    ws.onmessage = (e) => {
      try {
        const data = JSON.parse(e.data);
        if (data.type === "update") {
          setStatus(data.status);
          if (data.fleet) setFleet(data.fleet);
          if (data.searches) setSearches(data.searches);
          if (data.search_jobs) setSearchJobs(data.search_jobs);
          if (data.deployments) setDeployments(data.deployments);
          if (data.notifications) setNotifications(data.notifications);
          if (data.agent_tasks) setAgentTasks(data.agent_tasks);
          if (data.agent_budgets) setAgentBudgets(data.agent_budgets);
          if (data.running_agents) setRunningAgents(data.running_agents);
          if (data.projects) setProjects(data.projects);
          if (data.records) setRecords(data.records);
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
    searchJobs,
    deployments,
    notifications,
    agentTasks,
    agentBudgets,
    runningAgents,
    projects,
    records,
    connected,
    sendMessage,
  };
}
