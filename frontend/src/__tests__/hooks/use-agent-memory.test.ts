/**
 * @file Tests for useAgentMemory hook, upsertMemory, deleteMemory, and MEMORY_CATEGORIES
 * @module __tests__/hooks/use-agent-memory
 *
 * Validates the agent memory key-value store system used for persistent learning.
 * Agents store conventions, patterns, gotchas, and architectural decisions as
 * structured memories that persist across sessions. Tests cover the full CRUD
 * lifecycle (fetch, upsert, delete), Supabase Realtime subscriptions for live
 * updates, and the predefined memory category taxonomy.
 *
 * The mock chain simulates a double .order() call (category, then key) which
 * requires special handling since the first order returns the chain while the
 * second resolves with data.
 *
 * @see {@link ../../hooks/use-agent-memory} Source hook
 * @see {@link ../../__mocks__/supabase} Supabase mock configuration
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// --- Supabase mock ---
// Mocks the full Supabase query chain including CRUD operations (upsert, delete)
// and Realtime channel subscriptions for live memory updates.
const mockSelect = vi.fn();
const mockOrder = vi.fn();
const mockFrom = vi.fn();
const mockUpsert = vi.fn();
const mockDelete = vi.fn();
const mockEq = vi.fn();
const mockSingle = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();

/**
 * Configures the mock query chain with special double-order handling.
 * The useAgentMemory hook calls .order("category").order("key"), so the first
 * .order() must return the chain (for further chaining), while the second
 * .order() must resolve with the final data/error result.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  // For the second .order() call in the chain, resolve with data
  const orderResult = {
    data: finalData,
    error: finalError,
    then: vi.fn((resolve: (v: unknown) => void) =>
      resolve({ data: finalData, error: finalError })
    ),
  };

  const chain = {
    select: mockSelect.mockReturnThis(),
    order: mockOrder,
    upsert: mockUpsert.mockReturnThis(),
    delete: mockDelete.mockReturnThis(),
    eq: mockEq.mockReturnThis(),
    single: mockSingle.mockResolvedValue({ data: finalData, error: finalError }),
  };

  // First .order("category") returns chain (this), second .order("key") resolves
  let orderCallCount = 0;
  mockOrder.mockImplementation(() => {
    orderCallCount++;
    if (orderCallCount % 2 === 1) {
      // First order call returns chain
      return chain;
    }
    // Second order call resolves
    return orderResult;
  });

  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock: simulates supabase.channel().on().subscribe()
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

import { useAgentMemory, upsertMemory, deleteMemory, MEMORY_CATEGORIES } from "@/hooks/use-agent-memory";

// Tests the memory data fetching lifecycle with Realtime subscription.
// Validates that the hook queries the agent_memory table, orders results
// by category then key, subscribes to live updates, and cleans up on unmount.
describe("useAgentMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches memory entries on mount and exposes them
   * through the returned memories array. The mock simulates a successful
   * query returning one convention-type memory ("rust-style").
   *
   * Assertions: memories array populated, key and category match expected
   * values, loading completes, correct table queried.
   */
  it("fetches memories on mount", async () => {
    const mockData = [
      {
        id: 1,
        key: "rust-style",
        value: "Use snake_case for functions",
        category: "convention",
        created_by_task: null,
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.memories).toHaveLength(1);
    });
    expect(result.current.memories[0].key).toBe("rust-style");
    expect(result.current.memories[0].category).toBe("convention");
    expect(result.current.loading).toBe(false);
    expect(mockFrom).toHaveBeenCalledWith("agent_memory");
  });

  /**
   * Verifies graceful error handling when the agent_memory table is missing
   * or inaccessible. The hook should return an empty array rather than throw.
   */
  it("returns empty array on error", async () => {
    setupChain(null, { message: "Table not found" });

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.memories).toEqual([]);
  });

  /**
   * Verifies the initial loading state before async fetch completes.
   */
  it("starts with loading true", () => {
    setupChain([], null);

    const { result } = renderHook(() => useAgentMemory());

    expect(result.current.loading).toBe(true);
  });

  /**
   * Verifies that the hook subscribes to Supabase Realtime postgres_changes
   * on the agent_memory table. This enables live updates when memories are
   * created, updated, or deleted by agents or other clients.
   *
   * Assertions: channel created with correct name, subscribed to all events
   * (*) on the agent_memory table, subscribe() called.
   */
  it("subscribes to realtime changes", async () => {
    setupChain([], null);

    renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("agent_memory_changes");
    });
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "*", schema: "public", table: "agent_memory" },
      expect.any(Function)
    );
    expect(mockSubscribe).toHaveBeenCalled();
  });

  /**
   * Verifies that the Realtime channel is properly cleaned up when the
   * component using the hook unmounts. This prevents memory leaks and
   * stale subscription callbacks.
   */
  it("cleans up realtime subscription on unmount", async () => {
    setupChain([], null);

    const { unmount } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalled();
    });

    unmount();
    expect(mockRemoveChannel).toHaveBeenCalled();
  });

  /**
   * Verifies that the hook exposes a refetch function for manual re-fetch
   * after memory mutations performed outside the Realtime subscription.
   */
  it("provides a refetch function", async () => {
    setupChain([], null);

    const { result } = renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that memories are ordered by category first, then by key.
   * This ordering groups related memories together in the UI
   * (e.g., all "convention" entries before "gotcha" entries).
   */
  it("orders memories by category then key", async () => {
    setupChain([], null);

    renderHook(() => useAgentMemory());

    await waitFor(() => {
      expect(mockOrder).toHaveBeenCalledWith("category");
      expect(mockOrder).toHaveBeenCalledWith("key");
    });
  });
});

// Tests the upsertMemory standalone function which creates or updates a memory
// entry by key. Uses Supabase's upsert with onConflict: "key" for idempotent writes.
describe("upsertMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that upsertMemory inserts a new memory with the default "general"
   * category when no category is specified. The mock simulates the full
   * upsert chain: from().upsert().select().single().
   *
   * Assertions: correct table queried, upsert called with key/value/category,
   * onConflict set to "key" for idempotent behavior, returned data matches.
   */
  it("upserts a memory with default category", async () => {
    const returnData = {
      id: 1,
      key: "test-key",
      value: "test-value",
      category: "general",
      created_by_task: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: returnData, error: null }),
    };
    mockUpsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ upsert: mockUpsert });

    const result = await upsertMemory("test-key", "test-value");

    expect(mockFrom).toHaveBeenCalledWith("agent_memory");
    expect(mockUpsert).toHaveBeenCalledWith(
      expect.objectContaining({ key: "test-key", value: "test-value", category: "general" }),
      { onConflict: "key" }
    );
    expect(result.key).toBe("test-key");
  });

  /**
   * Verifies that upsertMemory throws the Supabase error when the upsert
   * operation fails (e.g., constraint conflict).
   */
  it("throws on error", async () => {
    const chain = {
      select: mockSelect.mockReturnThis(),
      single: mockSingle.mockResolvedValue({ data: null, error: { message: "Conflict" } }),
    };
    mockUpsert.mockReturnValue(chain);
    mockFrom.mockReturnValue({ upsert: mockUpsert });

    await expect(upsertMemory("key", "val")).rejects.toEqual({ message: "Conflict" });
  });
});

// Tests the deleteMemory standalone function which removes a memory entry
// by its unique key. Uses from().delete().eq("key", key).
describe("deleteMemory", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that deleteMemory correctly issues a DELETE query filtered
   * by key on the agent_memory table.
   */
  it("deletes a memory by key", async () => {
    mockEq.mockResolvedValue({ error: null });
    mockDelete.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ delete: mockDelete });

    await deleteMemory("test-key");

    expect(mockFrom).toHaveBeenCalledWith("agent_memory");
    expect(mockDelete).toHaveBeenCalled();
    expect(mockEq).toHaveBeenCalledWith("key", "test-key");
  });

  /**
   * Verifies that deleteMemory throws the Supabase error when the
   * delete operation fails (e.g., key not found or permission denied).
   */
  it("throws on delete error", async () => {
    mockEq.mockResolvedValue({ error: { message: "Not found" } });
    mockDelete.mockReturnValue({ eq: mockEq });
    mockFrom.mockReturnValue({ delete: mockDelete });

    await expect(deleteMemory("nonexistent")).rejects.toEqual({ message: "Not found" });
  });
});

// Tests the MEMORY_CATEGORIES constant which defines the taxonomy of memory types.
// These categories are used to organize agent knowledge by domain.
describe("MEMORY_CATEGORIES", () => {
  /**
   * Verifies that MEMORY_CATEGORIES exports exactly the 6 known categories:
   * pattern, convention, gotcha, preference, architecture, and general.
   * Adding or removing categories would require updating this test.
   */
  it("exports known categories", () => {
    expect(MEMORY_CATEGORIES).toContain("pattern");
    expect(MEMORY_CATEGORIES).toContain("convention");
    expect(MEMORY_CATEGORIES).toContain("gotcha");
    expect(MEMORY_CATEGORIES).toContain("preference");
    expect(MEMORY_CATEGORIES).toContain("architecture");
    expect(MEMORY_CATEGORIES).toContain("general");
    expect(MEMORY_CATEGORIES).toHaveLength(6);
  });
});
