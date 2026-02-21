/**
 * @file Tests for useAgentSchedules hook and schedule CRUD functions
 * @module __tests__/hooks/use-agent-schedules
 *
 * Validates the agent scheduling system which supports both cron-based and
 * event-driven triggers for automated agent tasks. Tests cover the full CRUD
 * lifecycle: list schedules, create, update, delete, and toggle enabled state.
 * Also validates Supabase Realtime subscriptions for live schedule updates.
 *
 * Schedule types:
 * - Cron triggers: time-based scheduling (e.g., "0 0 * * *" for nightly)
 * - Event triggers: reactive scheduling (e.g., fire on "prime_found" events)
 *
 * @see {@link ../../hooks/use-agent-schedules} Source hook and CRUD functions
 * @see {@link ../../__mocks__/supabase} Supabase mock configuration
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- Supabase mock ---
// Full CRUD mock chain for agent_schedules table operations.
const mockSelect = vi.fn();
const mockOrder = vi.fn();
const mockFrom = vi.fn();
const mockInsert = vi.fn();
const mockUpdate = vi.fn();
const mockDelete = vi.fn();
const mockEq = vi.fn();
const mockSingle = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();

/**
 * Configures the mock query chain to resolve with the given data/error.
 * Supports all CRUD operations: select, insert, update, delete.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  const chain = {
    select: mockSelect.mockReturnThis(),
    order: mockOrder.mockResolvedValue({
      data: finalData,
      error: finalError,
    }),
    insert: mockInsert.mockReturnThis(),
    update: mockUpdate.mockReturnThis(),
    delete: mockDelete.mockReturnThis(),
    eq: mockEq.mockReturnThis(),
    single: mockSingle.mockResolvedValue({ data: finalData, error: finalError }),
  };
  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock for live schedule change notifications
const channelMock = {
  on: mockOn.mockReturnThis(),
  subscribe: mockSubscribe.mockReturnThis(),
};
mockChannel.mockReturnValue(channelMock);

vi.mock("@/lib/supabase", () => ({
  supabase: {
    from: (...args: unknown[]) => mockFrom(...args),
    channel: (...args: unknown[]) => mockChannel(...args),
    removeChannel: (...args: unknown[]) => mockRemoveChannel(...args),
  },
}));

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
// Validates Supabase query construction, Realtime subscription, and cleanup.
describe("useAgentSchedules", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches schedules on mount and exposes them
   * through the returned schedules array. The mock simulates one
   * cron-triggered "nightly-analysis" schedule.
   */
  it("fetches schedules on mount", async () => {
    const mockData = [makeSchedule()];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.schedules).toHaveLength(1);
    });
    expect(result.current.schedules[0].name).toBe("nightly-analysis");
    expect(result.current.loading).toBe(false);
    expect(mockFrom).toHaveBeenCalledWith("agent_schedules");
  });

  /**
   * Verifies graceful error handling when the Supabase query fails.
   * Schedules defaults to an empty array.
   */
  it("returns empty array on error", async () => {
    setupChain(null, { message: "Permission denied" });

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.schedules).toEqual([]);
  });

  /** Verifies the initial loading state before async fetch completes. */
  it("starts with loading true", () => {
    setupChain([], null);

    const { result } = renderHook(() => useAgentSchedules());

    expect(result.current.loading).toBe(true);
  });

  /**
   * Verifies that the hook subscribes to Supabase Realtime postgres_changes
   * on the agent_schedules table for live updates when schedules are
   * created, modified, or deleted.
   */
  it("subscribes to realtime changes", async () => {
    setupChain([], null);

    renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("agent_schedules_changes");
    });
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "*", schema: "public", table: "agent_schedules" },
      expect.any(Function)
    );
    expect(mockSubscribe).toHaveBeenCalled();
  });

  /**
   * Verifies that the Realtime channel is cleaned up on unmount
   * to prevent memory leaks.
   */
  it("cleans up realtime subscription on unmount", async () => {
    setupChain([], null);

    const { unmount } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalled();
    });

    unmount();
    expect(mockRemoveChannel).toHaveBeenCalled();
  });

  /** Verifies that a refetch function is exposed for manual re-fetch. */
  it("provides a refetch function", async () => {
    setupChain([], null);

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
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentSchedules());

    await waitFor(() => {
      expect(result.current.schedules).toHaveLength(2);
    });
    expect(result.current.schedules[1].trigger_type).toBe("event");
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
   * Verifies that createSchedule inserts a schedule with required fields
   * and correct defaults (enabled=false, priority="normal", permission_level=1).
   * The mock chain simulates: from().insert().select().single().
   */
  it("creates a schedule with required fields", async () => {
    const returnData = makeSchedule({ id: 10 });
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockInsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ insert: mockInsert });

    const result = await createSchedule({
      name: "test-schedule",
      trigger_type: "cron",
      task_title: "Test task",
    });

    expect(mockFrom).toHaveBeenCalledWith("agent_schedules");
    expect(mockInsert).toHaveBeenCalledWith(
      expect.objectContaining({
        name: "test-schedule",
        trigger_type: "cron",
        task_title: "Test task",
        enabled: false,
        priority: "normal",
        permission_level: 1,
      })
    );
    expect(result.id).toBe(10);
  });

  /**
   * Verifies that createSchedule throws the Supabase error when insertion
   * fails (e.g., duplicate name constraint violation).
   */
  it("throws on insert error", async () => {
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: null, error: { message: "Duplicate name" } }),
    };
    mockInsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ insert: mockInsert });

    await expect(
      createSchedule({ name: "dup", trigger_type: "cron", task_title: "t" })
    ).rejects.toEqual({ message: "Duplicate name" });
  });
});

// Tests the updateSchedule function which patches specific fields on an
// existing schedule. The function also sets updated_at to the current timestamp.
describe("updateSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that updateSchedule sends a partial update with the given fields
   * and auto-sets updated_at. The mock chain simulates:
   * from().update().eq("id", id).select().single().
   */
  it("updates schedule fields", async () => {
    const returnData = makeSchedule({ name: "renamed" });
    const chain = {
      eq: mockEq.mockReturnThis(),
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockUpdate.mockReturnValue(chain);
    mockFrom.mockReturnValue({ update: mockUpdate });

    const result = await updateSchedule(1, { name: "renamed" });

    expect(mockFrom).toHaveBeenCalledWith("agent_schedules");
    expect(mockUpdate).toHaveBeenCalledWith(
      expect.objectContaining({ name: "renamed", updated_at: expect.any(String) })
    );
    expect(result.name).toBe("renamed");
  });

  /** Verifies that updateSchedule throws when the target schedule is not found. */
  it("throws on update error", async () => {
    const chain = {
      eq: mockEq.mockReturnThis(),
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: null, error: { message: "Not found" } }),
    };
    mockUpdate.mockReturnValue(chain);
    mockFrom.mockReturnValue({ update: mockUpdate });

    await expect(updateSchedule(999, { name: "x" })).rejects.toEqual({ message: "Not found" });
  });
});

// Tests the deleteSchedule function which removes a schedule by its numeric ID.
describe("deleteSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that deleteSchedule issues a DELETE filtered by id
   * on the agent_schedules table.
   */
  it("deletes a schedule by id", async () => {
    mockEq.mockResolvedValue({ error: null });
    mockDelete.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ delete: mockDelete });

    await deleteSchedule(1);

    expect(mockFrom).toHaveBeenCalledWith("agent_schedules");
    expect(mockDelete).toHaveBeenCalled();
    expect(mockEq).toHaveBeenCalledWith("id", 1);
  });

  /**
   * Verifies that deleteSchedule throws when a foreign key constraint
   * prevents deletion (e.g., schedule has associated task history).
   */
  it("throws on delete error", async () => {
    mockEq.mockResolvedValue({ error: { message: "FK constraint" } });
    mockDelete.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ delete: mockDelete });

    await expect(deleteSchedule(1)).rejects.toEqual({ message: "FK constraint" });
  });
});

// Tests the toggleSchedule convenience function which enables or disables
// a schedule by updating only the enabled field.
describe("toggleSchedule", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies toggling a schedule to enabled=true. The update payload
   * should contain { enabled: true } and the returned schedule should
   * reflect the new state.
   */
  it("toggles enabled to true", async () => {
    const returnData = makeSchedule({ enabled: true });
    const chain = {
      eq: mockEq.mockReturnThis(),
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockUpdate.mockReturnValue(chain);
    mockFrom.mockReturnValue({ update: mockUpdate });

    const result = await toggleSchedule(1, true);

    expect(mockUpdate).toHaveBeenCalledWith(
      expect.objectContaining({ enabled: true })
    );
    expect(result.enabled).toBe(true);
  });

  /**
   * Verifies toggling a schedule to enabled=false (disabling it).
   * This prevents the scheduler from firing the associated task.
   */
  it("toggles enabled to false", async () => {
    const returnData = makeSchedule({ enabled: false });
    const chain = {
      eq: mockEq.mockReturnThis(),
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockUpdate.mockReturnValue(chain);
    mockFrom.mockReturnValue({ update: mockUpdate });

    const result = await toggleSchedule(1, false);

    expect(mockUpdate).toHaveBeenCalledWith(
      expect.objectContaining({ enabled: false })
    );
    expect(result.enabled).toBe(false);
  });
});
