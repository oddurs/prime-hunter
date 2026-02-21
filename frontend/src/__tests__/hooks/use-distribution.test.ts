/**
 * @file Tests for useDistribution hook
 * @module __tests__/hooks/use-distribution
 *
 * Validates the digit distribution histogram hook which calls the REST API
 * endpoint `/api/stats/distribution` to aggregate prime counts by digit
 * count buckets and form type. Used to render the digit distribution chart
 * on the dashboard, showing how many primes exist in each digit range.
 *
 * @see {@link ../../hooks/use-distribution} Source hook
 * @see {@link ../../components/charts/digit-distribution} Chart component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock global fetch for REST API calls
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useDistribution } from "@/hooks/use-distribution";

// Tests the distribution data fetching lifecycle: mount -> fetch call -> data/error.
// Validates bucket size parameter handling and empty/error response behavior.
describe("useDistribution", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook calls the /api/stats/distribution endpoint on mount
   * with the default bucket size of 10 digits. The mock returns two buckets:
   * 0-9 digits (10 factorial primes) and 10-19 digits (5 factorial primes).
   */
  it("fetches distribution on mount with default bucket size", async () => {
    const mockData = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 10, form: "factorial", count: 5 },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(result.current.distribution).toHaveLength(2);
    });
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/stats/distribution?bucket_size=10")
    );
  });

  /**
   * Verifies that a custom bucket size (50) is correctly passed as a
   * query parameter, allowing the chart to show wider digit ranges.
   */
  it("passes custom bucket size", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    renderHook(() => useDistribution(50));

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining("/api/stats/distribution?bucket_size=50")
      );
    });
  });

  /** Verifies that an empty response results in an empty distribution array. */
  it("handles empty response", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });

  /** Verifies graceful error handling; distribution defaults to empty array. */
  it("returns empty on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });
});
