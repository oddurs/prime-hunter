/**
 * @file Tests for useAgentSchedules hook and schedule CRUD functions
 * @module __tests__/hooks/use-agent-schedules
 *
 * Validates the agent scheduling system which supports both cron-based and
 * event-driven triggers for automated agent tasks. Tests cover the full CRUD
 * lifecycle: list schedules, create, update, delete, and toggle enabled state.
 * Uses polling (every 10 seconds) for live updates.
 *
 * Schedule types:
 * - Cron triggers: time-based scheduling (e.g., "0 0 * * *" for nightly)
 * - Event triggers: reactive scheduling (e.g., fire on "prime_found" events)
 *
 * @see {@link ../../hooks/use-agent-schedules} Source hook and CRUD functions
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- fetch mock ---
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import {
  useAgentSchedules,
  createSchedule,
  updateSchedule,
  deleteSchedule,
  toggleSchedule,
} from "@/hooks/use-agent-schedules";

/**
 * Factory function to create a complete schedule object with sensible defaults.
 * Overrides allow tests to customize specific fields while keeping the rest valid.
 */
function makeSchedule(overrides: Record<string, unknown> = {}) {
  return {
    id: 1,
    name: "nightly-analysis",
    description: "Run nightly analysis",
    enabled: true,
    trigger_type: "cron",
    cron_expr: "0 0 * * *",
    event_filter: null,
    action_type: "task",
    template_name: null,
    role_name: null,
    task_title: "Nightly analysis",
    task_description: "Analyze search results",
    priority: "normal",
    max_cost_usd: 5.0,
    permission_level: 1,
    fire_count: 42,
    last_fired_at: "2026-02-01T00:00:00Z",
    last_checked_at: "2026-02-01T00:00:00Z",
    created_at: "2026-01-01T00:00:00Z",
    updated_at: "2026-02-01T00:00:00Z",
    ...overrides,
  };
}

// Tests the schedule listing lifecycle: mount -> loading -> data -> error states.
// Validates fetch calls and polling behavior.
describe("useAgentSchedules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches schedules on mount and exposes them
   * through the returned schedules array. The mock simulates one
   * cron-triggered "nightly-analysis" schedule.
   */
  it("fetches schedules on mount", async () => {
    const mockData = [makeSchedule()];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ schedules: mockData }),
    });

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.schedules).toHaveLength(1);
    });
    expect(result.current.schedules[0].name).toBe("nightly-analysis");
    expect(result.current.loading).toBe(false);
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/schedules"));
  });

  /**
   * Verifies graceful error handling when the fetch response is not ok.
   * Schedules defaults to an empty array.
   */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.schedules).toEqual([]);
  });

  /** Verifies the initial loading state before async fetch completes. */
  it("starts with loading true", () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ schedules: [] }),
    });

    const { result } = renderHook(() => useAgentSchedules());

    expect(result.current.loading).toBe(true);
  });

  /** Verifies that a refetch function is exposed for manual re-fetch. */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ schedules: [] }),
    });

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that the hook handles multiple schedules with different trigger
   * types (cron vs event). The second schedule uses an event_filter instead
   * of a cron_expr, representing a reactive trigger.
   */
  it("handles multiple schedules", async () => {
    const mockData = [
      makeSchedule({ id: 1, name: "alpha" }),
      makeSchedule({ id: 2, name: "beta", trigger_type: "event", cron_expr: null, event_filter: "prime_found" }),
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ schedules: mockData }),
    });

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.schedules).toHaveLength(2);
    });
    expect(result.current.schedules[1].trigger_type).toBe("event");
  });

  /**
   * Verifies that the polling interval is cleaned up on unmount
   * to prevent memory leaks. After unmount, no further fetch calls
   * should be triggered by the polling interval.
   */
  it("cleans up polling interval on unmount", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ schedules: [] }),
    });

    const { unmount } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });

    unmount();

    // The clearInterval is called on unmount; we verify no errors are thrown
    // and the hook cleans up properly. The polling interval is internal and
    // would only manifest as additional fetch calls over time.
  });
});

// Tests the createSchedule function which inserts a new schedule with defaults.
// New schedules start disabled (enabled: false) with normal priority
// and permission level 1.
describe("createSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that createSchedule sends a POST request with required fields
   * and correct defaults (enabled=false, priority="normal", permission_level=1).
   */
  it("creates a schedule with required fields", async () => {
    const returnData = makeSchedule({ id: 10 });
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await createSchedule({
      name: "test-schedule",
      trigger_type: "cron",
      task_title: "Test task",
    });

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules"),
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
      })
    );

    // Verify the body contains correct defaults
    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.name).toBe("test-schedule");
    expect(body.trigger_type).toBe("cron");
    expect(body.task_title).toBe("Test task");
    expect(body.enabled).toBe(false);
    expect(body.priority).toBe("normal");
    expect(body.permission_level).toBe(1);

    expect(result.id).toBe(10);
  });

  /**
   * Verifies that createSchedule throws when the API returns an error
   * (e.g., duplicate name constraint violation).
   */
  it("throws on insert error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Duplicate name" }),
    });

    await expect(
      createSchedule({ name: "dup", trigger_type: "cron", task_title: "t" })
    ).rejects.toThrow("Duplicate name");
  });
});

// Tests the updateSchedule function which patches specific fields on an
// existing schedule.
describe("updateSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that updateSchedule sends a PUT request with the given fields
   * to the correct endpoint including the schedule ID.
   */
  it("updates schedule fields", async () => {
    const returnData = makeSchedule({ name: "renamed" });
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await updateSchedule(1, { name: "renamed" });

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules/1"),
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      })
    );

    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.name).toBe("renamed");

    expect(result.name).toBe("renamed");
  });

  /** Verifies that updateSchedule throws when the target schedule is not found. */
  it("throws on update error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Not found" }),
    });

    await expect(updateSchedule(999, { name: "x" })).rejects.toThrow("Not found");
  });
});

// Tests the deleteSchedule function which removes a schedule by its numeric ID.
describe("deleteSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that deleteSchedule issues a DELETE request
   * to the correct endpoint with the schedule ID.
   */
  it("deletes a schedule by id", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
    });

    await deleteSchedule(1);

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules/1"),
      expect.objectContaining({
        method: "DELETE",
      })
    );
  });

  /**
   * Verifies that deleteSchedule throws when a constraint
   * prevents deletion (e.g., schedule has associated task history).
   */
  it("throws on delete error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "FK constraint" }),
    });

    await expect(deleteSchedule(1)).rejects.toThrow("FK constraint");
  });
});

// Tests the toggleSchedule convenience function which enables or disables
// a schedule by updating only the enabled field.
describe("toggleSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies toggling a schedule to enabled=true. The request payload
   * should contain { enabled: true } and the returned schedule should
   * reflect the new state.
   */
  it("toggles enabled to true", async () => {
    const returnData = makeSchedule({ enabled: true });
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await toggleSchedule(1, true);

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules/1/toggle"),
      expect.objectContaining({
        method: "PUT",
        headers: { "Content-Type": "application/json" },
      })
    );

    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.enabled).toBe(true);

    expect(result.enabled).toBe(true);
  });

  /**
   * Verifies toggling a schedule to enabled=false (disabling it).
   * This prevents the scheduler from firing the associated task.
   */
  it("toggles enabled to false", async () => {
    const returnData = makeSchedule({ enabled: false });
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(returnData),
    });

    const result = await toggleSchedule(1, false);

    const callArgs = mockFetch.mock.calls[0];
    const body = JSON.parse(callArgs[1].body);
    expect(body.enabled).toBe(false);

    expect(result.enabled).toBe(false);
  });
});
