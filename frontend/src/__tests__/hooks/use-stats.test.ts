/**
 * @file Tests for useStats hook
 * @module __tests__/hooks/use-stats
 *
 * Validates the dashboard statistics hook which calls the REST API
 * `/api/stats` endpoint to retrieve aggregate metrics: total prime count,
 * per-form breakdown, largest prime digits, and largest expression.
 * These stats are displayed in the stat cards on the main dashboard page.
 *
 * @see {@link ../../hooks/use-stats} Source hook
 * @see {@link ../../components/stat-card} Stat card component
 * @see {@link ../../app/page} Main dashboard page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock global fetch for REST API calls
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useStats } from "@/hooks/use-stats";

// Tests the stats data fetching lifecycle: mount -> fetch call -> data/error.
describe("useStats", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches aggregate stats on mount via the
   * /api/stats REST endpoint. The mock returns 42 total primes, 30 factorial,
   * largest at 1000 digits with expression "100!+1".
   */
  it("fetches stats on mount", async () => {
    const mockData = {
      total: 42,
      by_form: [{ form: "factorial", count: 30 }],
      largest_digits: 1000,
      largest_expression: "100!+1",
    };
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).toEqual(mockData);
    });
    expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining("/api/stats"));
  });

  /**
   * Verifies that a failed response results in null stats rather than throwing.
   * The dashboard should handle null stats by showing empty/placeholder cards.
   */
  it("returns null stats on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.stats).toBeNull();
  });

  /** Verifies that a manual refetch function is exposed for on-demand refresh. */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ total: 1, by_form: [], largest_digits: 0, largest_expression: null }),
    });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).not.toBeNull();
    });
    expect(typeof result.current.refetch).toBe("function");
  });
});
