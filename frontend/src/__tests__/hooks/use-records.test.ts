/**
 * @file Tests for useRecords hook
 * @module __tests__/hooks/use-records
 *
 * Validates the world records data hook which fetches known world records
 * for each prime form from the REST API. Records track the current
 * world-record holders (e.g., PrimeGrid for factorial primes) along with
 * our best results for comparison. Used by the record-comparison component
 * to show how close darkreach is to each world record.
 *
 * The hook uses `fetch()` to call:
 * - GET /api/records (list all world records)
 * - POST /api/records/refresh (trigger backend refresh from external sources)
 *
 * Polling is done via setInterval (every 30 seconds) instead of Supabase Realtime.
 *
 * @see {@link ../../hooks/use-records} Source hook
 * @see {@link ../../components/record-comparison} Record comparison component
 * @see {@link ../../app/leaderboard/page} Leaderboard page
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

import { useRecords, refreshRecords } from "@/hooks/use-records";

describe("useRecords", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
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
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockData),
    });

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.records).toHaveLength(1);
    });
    expect(result.current.records[0].expression).toBe("208003! - 1");
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/records")
    );
  });

  /**
   * Verifies graceful error handling; records defaults to empty array
   * and the error message is exposed via result.current.error.
   */
  it("returns empty on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.records).toEqual([]);
    expect(result.current.error).toContain("Failed to fetch records");
  });

  /** Verifies that a manual refetch function is exposed. */
  it("provides a refetch function", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    const { result } = renderHook(() => useRecords());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(typeof result.current.refetch).toBe("function");
  });

  /**
   * Verifies that records are polled every 30 seconds via setInterval
   * (replaces the old Supabase Realtime subscription).
   */
  it("polls for records every 30 seconds", async () => {
    vi.useFakeTimers();
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve([]),
    });

    renderHook(() => useRecords());

    // Flush the initial useEffect + fetch
    await vi.advanceTimersByTimeAsync(0);
    expect(mockFetch).toHaveBeenCalledTimes(1);

    // Advance timer by 30 seconds to trigger polling
    await vi.advanceTimersByTimeAsync(30_000);
    expect(mockFetch).toHaveBeenCalledTimes(2);

    // Advance another 30 seconds
    await vi.advanceTimersByTimeAsync(30_000);
    expect(mockFetch).toHaveBeenCalledTimes(3);

    vi.useRealTimers();
  });
});

describe("refreshRecords", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /** Verifies that refreshRecords sends a POST to the refresh endpoint. */
  it("sends POST to /api/records/refresh", async () => {
    mockFetch.mockResolvedValue({ ok: true });

    await refreshRecords();

    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/records/refresh"),
      expect.objectContaining({ method: "POST" })
    );
  });

  /** Verifies that refreshRecords throws on failure. */
  it("throws on error response", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      json: () => Promise.resolve({ error: "Refresh failed" }),
    });

    await expect(refreshRecords()).rejects.toThrow("Refresh failed");
  });
});
