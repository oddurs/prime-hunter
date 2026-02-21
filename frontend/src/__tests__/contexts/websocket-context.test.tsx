/**
 * @file Tests for WebSocketProvider context and useWs hook
 * @module __tests__/contexts/websocket-context
 *
 * Validates the WebSocket context which provides real-time coordination
 * data to the entire component tree. The context wraps either the WebSocket
 * transport (useWebSocket) or the HTTP polling fallback (usePolling),
 * selected at compile time based on environment configuration.
 *
 * Tests cover:
 * - useWs hook throwing when used outside WebSocketProvider
 * - WsData shape verification (all fields accessible)
 * - Multi-consumer context sharing (same reference)
 * - Provider rendering children correctly
 *
 * Both transport hooks are mocked to return static data, isolating
 * the context layer from actual network behavior.
 *
 * @see {@link ../../contexts/websocket-context} Source context
 * @see {@link ../../hooks/use-websocket} WebSocket transport
 * @see {@link ../../hooks/use-polling} HTTP polling fallback
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import React from "react";

// --- Mock useWebSocket and usePolling before imports ---
// WebSocket mock: simulates a disconnected state with empty data
const mockWsData = {
  status: null,
  fleet: null,
  coordinator: null,
  searches: [],
  searchJobs: [],
  deployments: [],
  notifications: [],
  agentTasks: [],
  agentBudgets: [],
  runningAgents: [],
  projects: [],
  records: [],
  connected: false,
  sendMessage: vi.fn(),
};

// Polling mock: simulates a connected state with active status
const mockPollingData = {
  status: { active: true, checkpoint: null },
  fleet: { workers: [], total_workers: 0, total_cores: 0, total_tested: 0, total_found: 0 },
  coordinator: null,
  searches: [],
  searchJobs: [],
  deployments: [],
  notifications: [],
  agentTasks: [],
  agentBudgets: [],
  runningAgents: [],
  projects: [],
  records: [],
  connected: true,
  sendMessage: vi.fn(),
};

vi.mock("@/hooks/use-websocket", () => ({
  useWebSocket: () => mockWsData,
}));

vi.mock("@/hooks/use-polling", () => ({
  usePolling: () => mockPollingData,
}));

// We need to test both WebSocketProvider and useWs
// Since the provider selection is compile-time, we test with default (WsProvider)
import { WebSocketProvider, useWs } from "@/contexts/websocket-context";

// Tests the useWs context hook which provides coordination data to consumers.
describe("useWs", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that useWs throws a descriptive error when called outside
   * of a WebSocketProvider. This prevents silent null context bugs.
   */
  it("throws when used outside WebSocketProvider", () => {
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => {
      renderHook(() => useWs());
    }).toThrow("useWs must be used within a WebSocketProvider");

    consoleSpy.mockRestore();
  });

  /**
   * Verifies that useWs returns the full WsData shape when used inside
   * a WebSocketProvider. All fields should be accessible with their
   * initial/default values from the underlying transport mock.
   */
  it("returns WsData when used inside WebSocketProvider", () => {
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <WebSocketProvider>{children}</WebSocketProvider>;
    }

    const { result } = renderHook(() => useWs(), { wrapper: Wrapper });

    expect(result.current).toBeDefined();
    expect(result.current.status).toBeNull();
    expect(result.current.connected).toBe(false);
    expect(typeof result.current.sendMessage).toBe("function");
  });

  /** Verifies fleet, searches, and searchJobs are provided from the transport. */
  it("provides fleet data from the underlying transport", () => {
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <WebSocketProvider>{children}</WebSocketProvider>;
    }

    const { result } = renderHook(() => useWs(), { wrapper: Wrapper });

    expect(result.current.fleet).toBeNull();
    expect(result.current.searches).toEqual([]);
    expect(result.current.searchJobs).toEqual([]);
  });

  /** Verifies agent-related data fields are provided from the transport. */
  it("provides agent data fields", () => {
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <WebSocketProvider>{children}</WebSocketProvider>;
    }

    const { result } = renderHook(() => useWs(), { wrapper: Wrapper });

    expect(result.current.agentTasks).toEqual([]);
    expect(result.current.agentBudgets).toEqual([]);
    expect(result.current.runningAgents).toEqual([]);
  });

  /** Verifies project and record data fields are provided from the transport. */
  it("provides project and record data", () => {
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <WebSocketProvider>{children}</WebSocketProvider>;
    }

    const { result } = renderHook(() => useWs(), { wrapper: Wrapper });

    expect(result.current.projects).toEqual([]);
    expect(result.current.records).toEqual([]);
  });

  /** Verifies deployment and notification data fields are provided from the transport. */
  it("provides deployment and notification data", () => {
    function Wrapper({ children }: { children: React.ReactNode }) {
      return <WebSocketProvider>{children}</WebSocketProvider>;
    }

    const { result } = renderHook(() => useWs(), { wrapper: Wrapper });

    expect(result.current.deployments).toEqual([]);
    expect(result.current.notifications).toEqual([]);
  });
});

// Tests the WebSocketProvider component rendering and context sharing.
describe("WebSocketProvider", () => {
  /**
   * Verifies that the provider renders its children and makes the
   * transport data accessible to them via the useWs hook.
   */
  it("renders children", () => {
    const TestChild = () => {
      const data = useWs();
      return <div data-testid="ws-status">{data.connected ? "connected" : "disconnected"}</div>;
    };

    const { container } = require("@testing-library/react").render(
      <WebSocketProvider>
        <TestChild />
      </WebSocketProvider>
    );

    expect(container.textContent).toBe("disconnected");
  });

  /**
   * Verifies that multiple consumers of useWs receive the exact same
   * data reference (referential equality), confirming proper React
   * context sharing without unnecessary re-renders.
   */
  it("provides the same data to multiple consumers", () => {
    const results: unknown[] = [];

    function Consumer1() {
      const data = useWs();
      results[0] = data;
      return null;
    }

    function Consumer2() {
      const data = useWs();
      results[1] = data;
      return null;
    }

    require("@testing-library/react").render(
      <WebSocketProvider>
        <Consumer1 />
        <Consumer2 />
      </WebSocketProvider>
    );

    expect(results[0]).toBe(results[1]);
  });
});
