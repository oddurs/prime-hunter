/**
 * @file Tests for useWebSocket hook
 * @module __tests__/hooks/use-websocket
 *
 * Comprehensive test suite for the WebSocket transport hook which provides
 * real-time coordination data from the Rust backend. This is the primary
 * transport for fleet status, search progress, deployments, agent data,
 * projects, records, and notifications. The hook manages the full WebSocket
 * lifecycle: connect, parse messages, track state, send commands, and cleanup.
 *
 * Message types handled:
 * - "update": Full state snapshot with status, fleet, searches, agents, etc.
 * - "notification": Prime discovery notifications (deduplicated, capped at 50)
 *
 * The mock uses a proper class-based WebSocket stub (not just an object)
 * to support the `new WebSocket(url)` constructor pattern used by the hook.
 *
 * @see {@link ../../hooks/use-websocket} Source hook
 * @see {@link ../../contexts/websocket-context} WebSocket context provider
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { useWebSocket } from "@/hooks/use-websocket";

/** Type definition for mock WebSocket instances tracked during tests. */
interface MockWsInstance {
  onopen: (() => void) | null;
  onclose: (() => void) | null;
  onerror: (() => void) | null;
  onmessage: ((e: { data: string }) => void) | null;
  close: ReturnType<typeof vi.fn>;
  send: ReturnType<typeof vi.fn>;
  readyState: number;
}

// Tests the full WebSocket lifecycle: connect -> receive -> parse -> send -> close.
// Each test creates a fresh MockWebSocket class to isolate connection state.
describe("useWebSocket", () => {
  /** Array tracking all WebSocket instances created during each test. */
  let wsInstances: MockWsInstance[];

  beforeEach(() => {
    wsInstances = [];

    // Use a proper class so `new WebSocket(...)` works.
    // Each instantiation is tracked in wsInstances for assertion.
    class MockWebSocket {
      static OPEN = 1;
      static CLOSED = 3;
      static CONNECTING = 0;
      static CLOSING = 2;

      onopen: (() => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: (() => void) | null = null;
      onmessage: ((e: { data: string }) => void) | null = null;
      close = vi.fn();
      send = vi.fn();
      readyState = 0;

      constructor() {
        wsInstances.push(this);
      }
    }

    vi.stubGlobal("WebSocket", MockWebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /** Helper to get the most recently created WebSocket instance. */
  function latestWs(): MockWsInstance {
    return wsInstances[wsInstances.length - 1];
  }

  /**
   * Verifies that the hook starts in a disconnected state with all data
   * fields at their default values (null/empty arrays).
   */
  it("initializes as disconnected", () => {
    const { result } = renderHook(() => useWebSocket());
    expect(result.current.connected).toBe(false);
    expect(result.current.fleet).toBeNull();
    expect(result.current.searches).toEqual([]);
  });

  /**
   * Verifies that the connected state transitions to true when the
   * WebSocket's onopen callback fires.
   */
  it("sets connected on WebSocket open", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    expect(result.current.connected).toBe(true);
  });

  /**
   * Verifies the connected -> disconnected transition when onclose fires.
   * This simulates server shutdown or network disconnection.
   */
  it("sets disconnected on WebSocket close", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });
    expect(result.current.connected).toBe(true);

    act(() => {
      latestWs().onclose?.();
    });
    expect(result.current.connected).toBe(false);
  });

  /**
   * Verifies parsing of "update" messages which contain the full coordinator
   * state snapshot. The hook should populate status, fleet, and searches
   * from the parsed JSON payload.
   */
  it("parses update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: true, checkpoint: null },
      fleet: {
        workers: [],
        total_workers: 3,
        total_cores: 24,
        total_tested: 1000,
        total_found: 5,
      },
      searches: [{ id: 1, search_type: "kbn", status: "running" }],
      deployments: [],
      notifications: [],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.status).toEqual({ active: true, checkpoint: null });
    expect(result.current.fleet?.total_workers).toBe(3);
    expect(result.current.searches).toHaveLength(1);
  });

  /**
   * Verifies parsing of "notification" messages which carry prime discovery
   * alerts. Notifications are appended to the notifications array.
   */
  it("handles notification messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const notifMsg = {
      type: "notification",
      notification: {
        id: 1,
        kind: "prime",
        title: "New prime!",
        details: ["5!+1"],
        count: 1,
        timestamp_ms: Date.now(),
      },
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notifMsg) });
    });

    expect(result.current.notifications).toHaveLength(1);
    expect(result.current.notifications[0].title).toBe("New prime!");
  });

  /**
   * Verifies that notifications are deduplicated by their numeric ID.
   * Sending the same notification twice should result in only one entry
   * in the notifications array.
   */
  it("deduplicates notifications by id", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const notif = {
      type: "notification",
      notification: {
        id: 1,
        kind: "prime",
        title: "New prime!",
        details: [],
        count: 1,
        timestamp_ms: Date.now(),
      },
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notif) });
    });
    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(notif) });
    });

    expect(result.current.notifications).toHaveLength(1);
  });

  /**
   * Verifies that non-JSON messages (e.g., "not-json") are silently
   * ignored without throwing or corrupting state. This guards against
   * proxy-injected health checks or malformed server output.
   */
  it("ignores malformed messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onmessage?.({ data: "not-json" });
    });

    expect(result.current.status).toBeNull();
  });

  /**
   * Verifies that sendMessage serializes the payload to JSON and calls
   * ws.send() when the WebSocket is in OPEN state (readyState=1).
   */
  it("sendMessage sends JSON when connected", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const ws = latestWs();
    ws.readyState = 1; // WebSocket.OPEN

    act(() => {
      ws.onopen?.();
    });

    act(() => {
      result.current.sendMessage({ type: "test" });
    });

    expect(ws.send).toHaveBeenCalledWith('{"type":"test"}');
  });

  /**
   * Verifies that sendMessage is a no-op when the WebSocket is not in
   * OPEN state (e.g., CLOSED with readyState=3). No data should be sent.
   */
  it("sendMessage does nothing when not open", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const ws = latestWs();
    ws.readyState = 3; // CLOSED

    act(() => {
      result.current.sendMessage({ type: "test" });
    });

    expect(ws.send).not.toHaveBeenCalled();
  });

  /**
   * Verifies that agent_tasks are extracted from update messages and
   * populated in the hook's state. This data feeds the agents page.
   */
  it("parses agent_tasks in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      agent_tasks: [
        { id: 1, title: "Analyze search", status: "running", priority: "high", agent_model: "claude-opus-4-20250514", tokens_used: 1000, cost_usd: 0.05, created_at: "2026-01-01" },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.agentTasks).toHaveLength(1);
    expect(result.current.agentTasks[0].title).toBe("Analyze search");
  });

  /**
   * Verifies that agent_budgets are extracted from update messages and
   * populated in the hook's state for cost tracking displays.
   */
  it("parses agent_budgets in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      agent_budgets: [
        { id: 1, period: "daily", budget_usd: 10, spent_usd: 3, tokens_used: 50000 },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.agentBudgets).toHaveLength(1);
    expect(result.current.agentBudgets[0].period).toBe("daily");
  });

  /**
   * Verifies that running_agents are extracted from update messages.
   * Running agents include task_id, model, and PID for process tracking.
   */
  it("parses running_agents in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      running_agents: [
        { task_id: 1, title: "Task 1", model: "claude-opus-4-20250514", status: "running", started_at: "2026-01-01", pid: 12345 },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.runningAgents).toHaveLength(1);
    expect(result.current.runningAgents[0].model).toBe("claude-opus-4-20250514");
  });

  /**
   * Verifies that projects are extracted from update messages for
   * campaign progress tracking on the projects page.
   */
  it("parses projects in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      projects: [
        { slug: "hunt-factorial", name: "Factorial Hunt", form: "factorial", objective: "Find 10K-digit factorial prime", status: "active", total_tested: 1000, total_found: 2, best_digits: 5000, total_cost_usd: 1.5 },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.projects).toHaveLength(1);
    expect(result.current.projects[0].slug).toBe("hunt-factorial");
  });

  /**
   * Verifies that world records are extracted from update messages
   * for the leaderboard comparison view.
   */
  it("parses records in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      records: [
        { form: "factorial", expression: "100!+1", digits: 158, holder: "darkreach", our_best_digits: 158 },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.records).toHaveLength(1);
    expect(result.current.records[0].form).toBe("factorial");
  });

  /**
   * Verifies that search_jobs are extracted from update messages.
   * Search jobs track individual search execution with range, progress,
   * and result counts.
   */
  it("parses search_jobs in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      search_jobs: [
        { id: 1, search_type: "kbn", params: {}, status: "running", error: null, created_at: "2026-01-01", started_at: "2026-01-01", stopped_at: null, range_start: 0, range_end: 1000, block_size: 100, total_tested: 500, total_found: 2 },
      ],
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.searchJobs).toHaveLength(1);
    expect(result.current.searchJobs[0].status).toBe("running");
  });

  /**
   * Verifies that coordinator system metrics (CPU, memory, disk, load)
   * are extracted from update messages for the metrics bar display.
   */
  it("parses coordinator metrics in update messages", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const metrics = {
      cpu_usage_percent: 45.5,
      memory_used_gb: 2.1,
      memory_total_gb: 8.0,
      memory_usage_percent: 26.3,
      disk_used_gb: 50,
      disk_total_gb: 200,
      disk_usage_percent: 25,
      load_avg_1m: 1.5,
      load_avg_5m: 1.2,
      load_avg_15m: 1.0,
    };

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
      coordinator: metrics,
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.coordinator).toEqual(metrics);
  });

  /**
   * Verifies that coordinator is set to null when the update message
   * does not include coordinator metrics (e.g., metrics collection disabled).
   */
  it("sets coordinator to null when not present in update", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    act(() => {
      latestWs().onopen?.();
    });

    const updateMsg = {
      type: "update",
      status: { active: false, checkpoint: null },
    };

    act(() => {
      latestWs().onmessage?.({ data: JSON.stringify(updateMsg) });
    });

    expect(result.current.coordinator).toBeNull();
  });

  /**
   * Verifies that the notification buffer is capped at 50 entries.
   * Sending 55 unique notifications should result in at most 50 being
   * retained, preventing unbounded memory growth during active searches.
   */
  it("limits notifications to 50", async () => {
    const { result } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    // Send 55 unique notifications
    for (let i = 0; i < 55; i++) {
      act(() => {
        latestWs().onmessage?.({
          data: JSON.stringify({
            type: "notification",
            notification: {
              id: i,
              kind: "prime",
              title: `Prime ${i}`,
              details: [],
              count: 1,
              timestamp_ms: Date.now() + i,
            },
          }),
        });
      });
    }

    expect(result.current.notifications.length).toBeLessThanOrEqual(50);
  });

  /**
   * Verifies that the WebSocket connection is closed on component unmount
   * to prevent resource leaks and orphaned connections.
   */
  it("closes WebSocket on unmount", async () => {
    const { unmount } = renderHook(() => useWebSocket());

    await waitFor(() => {
      expect(wsInstances.length).toBeGreaterThan(0);
    });

    const ws = latestWs();
    unmount();

    expect(ws.close).toHaveBeenCalled();
  });

  /**
   * Comprehensive check that all state fields are initialized to their
   * expected default values. This ensures no field is accidentally
   * undefined, which would cause runtime errors in consuming components.
   */
  it("initializes all state fields correctly", () => {
    const { result } = renderHook(() => useWebSocket());

    expect(result.current.status).toBeNull();
    expect(result.current.fleet).toBeNull();
    expect(result.current.coordinator).toBeNull();
    expect(result.current.searches).toEqual([]);
    expect(result.current.searchJobs).toEqual([]);
    expect(result.current.deployments).toEqual([]);
    expect(result.current.notifications).toEqual([]);
    expect(result.current.agentTasks).toEqual([]);
    expect(result.current.agentBudgets).toEqual([]);
    expect(result.current.runningAgents).toEqual([]);
    expect(result.current.projects).toEqual([]);
    expect(result.current.records).toEqual([]);
    expect(result.current.connected).toBe(false);
    expect(typeof result.current.sendMessage).toBe("function");
  });
});
