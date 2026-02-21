/**
 * @file Tests for useStats hook
 * @module __tests__/hooks/use-stats
 *
 * Validates the dashboard statistics hook which calls the Supabase RPC
 * function `get_stats` to retrieve aggregate metrics: total prime count,
 * per-form breakdown, largest prime digits, and largest expression.
 * These stats are displayed in the stat cards on the main dashboard page.
 *
 * @see {@link ../../hooks/use-stats} Source hook
 * @see {@link ../../components/stat-card} Stat card component
 * @see {@link ../../app/page} Main dashboard page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock the Supabase RPC method for stats retrieval
const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useStats } from "@/hooks/use-stats";

// Tests the stats data fetching lifecycle: mount -> RPC call -> data/error.
describe("useStats", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches aggregate stats on mount via the
   * get_stats RPC. The mock returns 42 total primes, 30 factorial,
   * largest at 1000 digits with expression "100!+1".
   */
  it("fetches stats on mount", async () => {
    const mockData = {
      total: 42,
      by_form: [{ form: "factorial", count: 30 }],
      largest_digits: 1000,
      largest_expression: "100!+1",
    };
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).toEqual(mockData);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_stats");
  });

  /**
   * Verifies that an RPC error results in null stats rather than throwing.
   * The dashboard should handle null stats by showing empty/placeholder cards.
   */
  it("returns null stats on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.stats).toBeNull();
  });

  /** Verifies that a manual refetch function is exposed for on-demand refresh. */
  it("provides a refetch function", async () => {
    mockRpc.mockResolvedValue({ data: { total: 1, by_form: [], largest_digits: 0, largest_expression: null }, error: null });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).not.toBeNull();
    });
    expect(typeof result.current.refetch).toBe("function");
  });
});
