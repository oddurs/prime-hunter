/**
 * @file Tests for useDistribution hook
 * @module __tests__/hooks/use-distribution
 *
 * Validates the digit distribution histogram hook which calls the Supabase
 * RPC function `get_digit_distribution` to aggregate prime counts by digit
 * count buckets and form type. Used to render the digit distribution chart
 * on the dashboard, showing how many primes exist in each digit range.
 *
 * @see {@link ../../hooks/use-distribution} Source hook
 * @see {@link ../../components/charts/digit-distribution} Chart component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock the Supabase RPC method used by the distribution hook
const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useDistribution } from "@/hooks/use-distribution";

// Tests the distribution data fetching lifecycle: mount -> RPC call -> data/error.
// Validates bucket size parameter handling and empty/error response behavior.
describe("useDistribution", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook calls the get_digit_distribution RPC on mount
   * with the default bucket size of 10 digits. The mock returns two buckets:
   * 0-9 digits (10 factorial primes) and 10-19 digits (5 factorial primes).
   */
  it("fetches distribution on mount with default bucket size", async () => {
    const mockData = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 10, form: "factorial", count: 5 },
    ];
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(result.current.distribution).toHaveLength(2);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_digit_distribution", {
      bucket_size_param: 10,
    });
  });

  /**
   * Verifies that a custom bucket size (50) is correctly passed to the
   * RPC function, allowing the chart to show wider digit ranges.
   */
  it("passes custom bucket size", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    renderHook(() => useDistribution(50));

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalledWith("get_digit_distribution", {
        bucket_size_param: 50,
      });
    });
  });

  /** Verifies that an empty RPC response results in an empty distribution array. */
  it("handles empty response", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });

  /** Verifies graceful error handling; distribution defaults to empty array. */
  it("returns empty on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });
});
