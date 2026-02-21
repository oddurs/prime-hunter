/**
 * @file Tests for usePolling hook
 * @module __tests__/hooks/use-polling
 *
 * Validates the HTTP polling fallback transport hook which provides the same
 * data interface as the WebSocket hook but via periodic HTTP fetches. This
 * fallback is used when WebSocket connections are unavailable (e.g., behind
 * restrictive firewalls or proxies that don't support WebSocket upgrades).
 *
 * The hook polls `/api/ws-snapshot` every 4 seconds, parsing the same JSON
 * payload format as WebSocket update messages. Tests cover the fetch lifecycle,
 * polling interval setup/cleanup, connection state tracking, error handling,
 * and data population from successful responses.
 *
 * @see {@link ../../hooks/use-polling} Source hook
 * @see {@link ../../hooks/use-websocket} Primary WebSocket transport
 * @see {@link ../../contexts/websocket-context} Transport selection logic
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock the WebSocket hook module to prevent actual WebSocket connections
vi.mock("@/hooks/use-websocket", () => ({}));

import { usePolling } from "@/hooks/use-polling";

/**
 * Factory for creating a mock successful HTTP response with all required
 * snapshot fields. Overrides allow tests to inject specific data while
 * keeping the rest at empty/default values.
 */
function makeSuccessResponse(overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    json: () =>
      Promise.resolve({
        type: "update",
        status: { active: false, checkpoint: null },
        fleet: {
          workers: [],
          total_workers: 0,
          total_cores: 0,
          total_tested: 0,
          total_found: 0,
        },
        searches: [],
        search_jobs: [],
        deployments: [],
        notifications: [],
        agent_tasks: [],
        agent_budgets: [],
        running_agents: [],
        projects: [],
        records: [],
        coordinator: null,
        ...overrides,
      }),
  };
}

// Tests the polling transport lifecycle: mount -> fetch -> parse -> interval -> cleanup.
// Validates connection state tracking, error resilience, data population,
// and the no-op sendMessage implementation.
describe("usePolling", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(makeSuccessResponse()));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /**
   * Verifies that the hook issues an immediate fetch to /api/ws-snapshot
   * on mount, before the polling interval kicks in.
   */
  it("calls fetch on mount", async () => {
    renderHook(() => usePolling());

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledTimes(1);
    });
    expect(fetch).toHaveBeenCalledWith("/api/ws-snapshot");
  });

  /**
   * Verifies that a 4-second polling interval is set up after mount.
   * The interval calls fetch periodically to keep data fresh.
   */
  it("sets up polling interval", async () => {
    const setIntervalSpy = vi.spyOn(global, "setInterval");

    renderHook(() => usePolling());

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledTimes(1);
    });

    expect(setIntervalSpy).toHaveBeenCalledWith(expect.any(Function), 4000);
    setIntervalSpy.mockRestore();
  });

  /**
   * Verifies that a successful HTTP response transitions the connected
   * state to true, indicating the polling transport is operational.
   */
  it("sets connected to true on successful response", async () => {
    const { result } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(result.current.connected).toBe(true);
    });
  });

  /**
   * Verifies that a network error (fetch rejection) leaves the connected
   * state as false. The hook should not throw but gracefully degrade.
   */
  it("sets connected to false on fetch error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new Error("Network error"))
    );

    const { result } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(fetch).toHaveBeenCalled();
    });
    // After the error, connected should remain false (initial state)
    expect(result.current.connected).toBe(false);
  });

  /**
   * Verifies that an HTTP error response (non-2xx) also sets connected
   * to false, distinguishing from successful responses.
   */
  it("sets connected to false on non-ok response", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({ ok: false })
    );

    const { result } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(fetch).toHaveBeenCalled();
    });
    expect(result.current.connected).toBe(false);
  });

  /**
   * Verifies that the hook populates all data fields from a successful
   * snapshot response: status, fleet, searches, projects, records,
   * and coordinator metrics. This ensures parity with the WebSocket
   * transport's data shape.
   */
  it("populates data from update response", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        json: () =>
          Promise.resolve({
            type: "update",
            status: { active: true, checkpoint: null },
            fleet: {
              workers: [{ worker_id: "w1" }],
              total_workers: 1,
              total_cores: 8,
              total_tested: 5000,
              total_found: 10,
            },
            searches: [{ id: 1 }],
            search_jobs: [{ id: 1 }],
            deployments: [],
            notifications: [],
            agent_tasks: [],
            agent_budgets: [],
            running_agents: [],
            projects: [{ slug: "p1" }],
            records: [{ id: 1 }],
            coordinator: { cpu_usage_percent: 50 },
          }),
      })
    );

    const { result } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(result.current.connected).toBe(true);
    });
    expect(result.current.status).toEqual({ active: true, checkpoint: null });
    expect(result.current.fleet?.total_workers).toBe(1);
    expect(result.current.searches).toHaveLength(1);
    expect(result.current.projects).toHaveLength(1);
    expect(result.current.records).toHaveLength(1);
    expect(result.current.coordinator?.cpu_usage_percent).toBe(50);
  });

  /**
   * Verifies that sendMessage is a no-op function. The polling transport
   * is read-only (HTTP GET), so sending messages is not supported.
   * The function exists for API compatibility with the WebSocket transport.
   */
  it("sendMessage is a no-op", async () => {
    const { result } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(result.current.connected).toBe(true);
    });

    // sendMessage should exist and not throw
    expect(typeof result.current.sendMessage).toBe("function");
    expect(() => result.current.sendMessage()).not.toThrow();
  });

  /**
   * Verifies that the polling interval is cleared on unmount to prevent
   * memory leaks and stale fetch calls after component removal.
   */
  it("cleans up interval on unmount", async () => {
    const clearIntervalSpy = vi.spyOn(global, "clearInterval");

    const { unmount } = renderHook(() => usePolling());

    await waitFor(() => {
      expect(fetch).toHaveBeenCalledTimes(1);
    });

    unmount();

    expect(clearIntervalSpy).toHaveBeenCalled();
    clearIntervalSpy.mockRestore();
  });
});
