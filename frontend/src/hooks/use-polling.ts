"use client";

/**
 * @module use-polling
 *
 * REST polling alternative to `use-websocket.ts`. Polls `/api/ws-snapshot`
 * every 4 seconds and returns the same `WsData` interface, so consumers
 * (via `useWs()`) work identically regardless of transport.
 *
 * Used when the frontend is hosted on Vercel: Vercel rewrites can proxy
 * REST calls to Hetzner but cannot proxy WebSocket connections.
 * Activated by setting `NEXT_PUBLIC_USE_POLLING=true`.
 *
 * `sendMessage` is a no-op since the WebSocket was read-only anyway.
 */

import { useEffect, useRef, useState, useCallback } from "react";
import type {
  WsData,
  Status,
  FleetData,
  HardwareMetrics,
  ManagedSearch,
  SearchJob,
  Deployment,
  Notification,
  AgentTaskSummary,
  AgentBudgetSummary,
  AgentInfo,
  ProjectSummary,
  RecordSummary,
} from "./use-websocket";

const POLL_INTERVAL_MS = 4000;

export function usePolling(): WsData {
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
  const intervalRef = useRef<ReturnType<typeof setInterval>>(undefined);

  const poll = useCallback(async () => {
    try {
      const res = await fetch("/api/ws-snapshot");
      if (!res.ok) {
        setConnected(false);
        return;
      }
      const data = await res.json();
      setConnected(true);
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
      }
    } catch {
      setConnected(false);
    }
  }, []);

  useEffect(() => {
    poll();
    intervalRef.current = setInterval(poll, POLL_INTERVAL_MS);
    return () => clearInterval(intervalRef.current);
  }, [poll]);

  const sendMessage = useCallback(() => {
    // no-op: polling mode is read-only
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
