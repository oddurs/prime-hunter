/**
 * @file Tests for useTimeline hook
 * @module __tests__/hooks/use-timeline
 *
 * Validates the discovery timeline hook which calls the Supabase RPC function
 * `get_discovery_timeline` to aggregate prime discovery counts over time
 * periods (day, hour, week). Used by the DiscoveryTimeline chart component
 * to visualize the rate of prime discoveries.
 *
 * @see {@link ../../hooks/use-timeline} Source hook
 * @see {@link ../../components/charts/discovery-timeline} Timeline chart component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock the Supabase RPC method for timeline data
const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useTimeline } from "@/hooks/use-timeline";

// Tests the timeline data fetching lifecycle: mount -> RPC call -> data/error.
// Validates bucket_type parameter handling and error response behavior.
describe("useTimeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook calls the get_discovery_timeline RPC on mount
   * with the default bucket_type of "day". The mock returns two daily
   * buckets with factorial prime counts.
   */
  it("fetches timeline on mount with default bucket type", async () => {
    const mockData = [
      { bucket: "2026-01-01", form: "factorial", count: 5 },
      { bucket: "2026-01-02", form: "factorial", count: 3 },
    ];
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(result.current.timeline).toHaveLength(2);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_discovery_timeline", {
      bucket_type: "day",
    });
  });

  /**
   * Verifies that a custom bucket_type ("hour") is correctly passed to the
   * RPC, allowing more granular timeline views during active searches.
   */
  it("passes custom bucket type", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    renderHook(() => useTimeline("hour"));

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalledWith("get_discovery_timeline", {
        bucket_type: "hour",
      });
    });
  });

  /** Verifies graceful error handling; timeline defaults to empty array. */
  it("returns empty array on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.timeline).toEqual([]);
  });
});
