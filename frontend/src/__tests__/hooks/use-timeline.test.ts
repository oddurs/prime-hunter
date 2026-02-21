/**
 * @file Tests for useTimeline hook
 * @module __tests__/hooks/use-timeline
 *
 * Validates the discovery timeline hook which calls the REST API endpoint
 * `/api/stats/timeline` to aggregate prime discovery counts over time
 * periods (day, hour, week). Used by the DiscoveryTimeline chart component
 * to visualize the rate of prime discoveries.
 *
 * @see {@link ../../hooks/use-timeline} Source hook
 * @see {@link ../../components/charts/discovery-timeline} Timeline chart component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock global fetch for REST API calls
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useTimeline } from "@/hooks/use-timeline";

// Tests the timeline data fetching lifecycle: mount -> fetch call -> data/error.
// Validates bucket_type parameter handling and error response behavior.
describe("useTimeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook calls the /api/stats/timeline endpoint on mount
   * with the default bucket_type of "day". The mock returns two daily
   * buckets with factorial prime counts.
   */
  it("fetches timeline on mount with default bucket type", async () => {
    const mockData = [
      { bucket: "2026-01-01", form: "factorial", count: 5 },
      { bucket: "2026-01-02", form: "factorial", count: 3 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(result.current.timeline).toHaveLength(2);
    });
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/stats/timeline?bucket_type=day")
    );
  });

  /**
   * Verifies that a custom bucket_type ("hour") is correctly passed as a
   * query parameter, allowing more granular timeline views during active searches.
   */
  it("passes custom bucket type", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    renderHook(() => useTimeline("hour"));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("/api/stats/timeline?bucket_type=hour")
      );
    });
  });

  /** Verifies graceful error handling; timeline defaults to empty array. */
  it("returns empty array on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.timeline).toEqual([]);
  });
});
