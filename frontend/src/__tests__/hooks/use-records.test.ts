/**
 * @file Tests for useRecords hook
 * @module __tests__/hooks/use-records
 *
 * Validates the world records data hook which fetches known world records
 * for each prime form from the "records" table. Records track the current
 * world-record holders (e.g., PrimeGrid for factorial primes) along with
 * our best results for comparison. Used by the record-comparison component
 * to show how close darkreach is to each world record.
 *
 * The mock chain uses a double .order() pattern (by form, then by digits)
 * similar to use-agent-memory, where the first order returns the chain
 * and the second resolves with data.
 *
 * @see {@link ../../hooks/use-records} Source hook
 * @see {@link ../../components/record-comparison} Record comparison component
 * @see {@link ../../app/leaderboard/page} Leaderboard page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock supabase with double-order chain pattern
const mockSelect = vi.fn();
const mockOrder = vi.fn();
const mockFrom = vi.fn();
const mockChannel = vi.fn();
const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockRemoveChannel = vi.fn();

/**
 * Configures the mock chain with double .order() support.
 * Both order calls return a thenable for resolution.
 */
function setupChain(finalData: unknown, finalError: unknown) {
  const chain = {
    select: mockSelect.mockReturnThis(),
    order: mockOrder.mockReturnValue({
      order: mockOrder,
      then: vi.fn((resolve: (v: unknown) => void) =>
        resolve({ data: finalData, error: finalError })
      ),
    }),
  };
  // The second .order() call returns a thenable
  mockOrder.mockReturnValue({
    order: mockOrder,
    then: vi.fn((resolve: (v: unknown) => void) =>
      resolve({ data: finalData, error: finalError })
    ),
  });
  mockFrom.mockReturnValue(chain);
  return chain;
}

// Realtime channel mock for live record updates
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

vi.mock("@/lib/format", () => ({
  API_BASE: "http://localhost:3000",
}));

import { useRecords } from "@/hooks/use-records";

// Tests the world records data lifecycle: mount -> loading -> data/error -> Realtime.
describe("useRecords", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockChannel.mockReturnValue(channelMock);
    mockOn.mockReturnValue(channelMock);
    mockSubscribe.mockReturnValue(channelMock);
  });

  /**
   * Verifies that the hook fetches world records on mount. The mock returns
   * one factorial record held by PrimeGrid with 208003!-1 at 1,015,843 digits,
   * sourced from T5K (Top 5000 Primes).
   */
  it("fetches records on mount", async () => {
    const mockData = [
      {
        id: 1,
        form: "factorial",
        category: "largest_known",
        expression: "208003! - 1",
        digits: 1015843,
        holder: "PrimeGrid",
        discovered_at: "2023-01-01T00:00:00Z",
        source: "T5K",
        source_url: "https://t5k.org/primes/page.php?id=123",
        our_best_id: null,
        our_best_digits: null,
        fetched_at: "2026-01-15T00:00:00Z",
        updated_at: "2026-01-15T00:00:00Z",
      },
    ];
    setupChain(mockData, null);

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.records).toHaveLength(1);
    });
    expect(result.current.records[0].expression).toBe("208003! - 1");
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(mockFrom).toHaveBeenCalledWith("records");
  });

  /**
   * Verifies graceful error handling; records defaults to empty array
   * and the error message is exposed via result.current.error.
   */
  it("returns empty on error", async () => {
    setupChain(null, { message: "Table not found" });

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.records).toEqual([]);
    expect(result.current.error).toBe("Table not found");
  });

  /** Verifies that a manual refetch function is exposed. */
  it("provides a refetch function", async () => {
    setupChain([], null);

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies Realtime subscription to all events on the records table
   * for live updates when world records are refreshed or our best improves.
   */
  it("subscribes to realtime changes", async () => {
    setupChain([], null);

    renderHook(() => useRecords());

    await waitFor(() => {
      expect(mockChannel).toHaveBeenCalledWith("records_changes");
    });
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "*", schema: "public", table: "records" },
      expect.any(Function)
    );
    expect(mockSubscribe).toHaveBeenCalled();
  });
});
