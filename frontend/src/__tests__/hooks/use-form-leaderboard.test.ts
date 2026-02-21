/**
 * @file Tests for useFormLeaderboard hook
 * @module __tests__/hooks/use-form-leaderboard
 *
 * Validates the form leaderboard hook which ranks prime number forms
 * (factorial, kbn, palindromic, etc.) by count, largest digits, and
 * verification percentage. Calls the REST API endpoint
 * `/api/stats/leaderboard` and auto-refreshes via a 10-second polling interval.
 *
 * @see {@link ../../hooks/use-form-leaderboard} Source hook
 * @see {@link ../../components/form-leaderboard} Leaderboard table component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

// Mock global fetch for REST API calls
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

import { useFormLeaderboard } from "@/hooks/use-form-leaderboard";

// Tests the leaderboard data fetching, polling interval setup, and cleanup.
describe("useFormLeaderboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches leaderboard data on mount via the
   * /api/stats/leaderboard REST endpoint. The mock returns two form entries:
   * factorial (150 primes, largest 45678 digits, 93% verified) and
   * kbn (80 primes, largest 12345 digits, 94% verified).
   *
   * Assertions: entries array populated, forms in expected order,
   * correct endpoint called.
   */
  it("fetches leaderboard data on mount", async () => {
    const mockData = [
      {
        form: "factorial",
        count: 150,
        largest_digits: 45678,
        largest_expression: "100000!+1",
        latest_found_at: "2026-02-01T00:00:00Z",
        verified_count: 140,
        verified_pct: 93,
      },
      {
        form: "kbn",
        count: 80,
        largest_digits: 12345,
        largest_expression: "3*2^41000+1",
        latest_found_at: "2026-02-10T00:00:00Z",
        verified_count: 75,
        verified_pct: 94,
      },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useFormLeaderboard());

    await waitFor(() => {
      expect(result.current.entries).toHaveLength(2);
    });
    expect(result.current.entries[0].form).toBe("factorial");
    expect(result.current.entries[1].form).toBe("kbn");
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/stats/leaderboard")
    );
  });

  /** Verifies graceful error handling; entries defaults to empty array. */
  it("returns empty on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useFormLeaderboard());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.entries).toEqual([]);
  });

  /** Verifies that a manual refetch function is exposed. */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useFormLeaderboard());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that the hook sets up a 10-second polling interval for
   * automatic data refresh. This keeps the leaderboard up to date
   * without requiring a full page reload.
   */
  it("sets up a polling interval", async () => {
    const setIntervalSpy = vi.spyOn(global, "setInterval");
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { unmount } = renderHook(() => useFormLeaderboard());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });

    // Verify setInterval was called with 10s interval
    expect(setIntervalSpy).toHaveBeenCalledWith(expect.any(Function), 10000);

    unmount();
    setIntervalSpy.mockRestore();
  });

  /**
   * Verifies that the polling interval is properly cleared on unmount
   * to prevent memory leaks and stale callbacks.
   */
  it("clears interval on unmount", async () => {
    const clearIntervalSpy = vi.spyOn(global, "clearInterval");
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { unmount } = renderHook(() => useFormLeaderboard());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });

    unmount();

    expect(clearIntervalSpy).toHaveBeenCalled();
    clearIntervalSpy.mockRestore();
  });
});
