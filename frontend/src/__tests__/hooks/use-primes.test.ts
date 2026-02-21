/**
 * @file Tests for usePrimes hook
 * @module __tests__/hooks/use-primes
 *
 * Validates the prime records data hook which provides paginated, filterable
 * access to primes via the REST API. This is the primary data hook for
 * the browse page and primes table component, supporting filters by form type,
 * expression search, digit range, and paginated queries.
 *
 * The hook uses `fetch()` to call the Rust backend REST API endpoints:
 * - GET /api/primes?limit=N&offset=N&form=...&search=...
 * - GET /api/primes/:id (detail view)
 *
 * @see {@link ../../hooks/use-primes} Source hook
 * @see {@link ../../components/primes-table} Primes table component
 * @see {@link ../../app/browse/page} Browse page
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

import { usePrimes } from "@/hooks/use-primes";

describe("usePrimes", () => {
  let mockFetch: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    mockFetch = vi.fn();
    vi.stubGlobal("fetch", mockFetch);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /**
   * Verifies that the hook fetches primes from the REST API on mount.
   * The mock returns one factorial prime (5!+1 = 121, 3 digits).
   */
  it("fetches primes on mount", async () => {
    const mockData = [
      {
        id: 1,
        form: "factorial",
        expression: "5!+1",
        digits: 3,
        found_at: "2026-01-01T00:00:00Z",
        proof_method: "deterministic",
        verified: false,
        verified_at: null,
        verification_method: null,
        verification_tier: null,
      },
    ];
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ primes: mockData, total: 1, limit: 50, offset: 0 }),
    });

    const { result } = renderHook(() => usePrimes());

    await waitFor(() => {
      expect(result.current.primes.primes).toHaveLength(1);
    });
    expect(result.current.primes.total).toBe(1);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/primes?")
    );
    // Verify default pagination params
    const calledUrl = mockFetch.mock.calls[0][0] as string;
    expect(calledUrl).toContain("limit=50");
    expect(calledUrl).toContain("offset=0");
  });

  /**
   * Verifies graceful error handling; primes array stays empty and no
   * exception is thrown to the consuming component.
   */
  it("returns empty data on error", async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 500,
    });

    const { result } = renderHook(() => usePrimes());

    await waitFor(() => {
      expect(mockFetch).toHaveBeenCalled();
    });
    expect(result.current.primes.primes).toHaveLength(0);
  });

  /**
   * Verifies that the form filter is passed as a URL parameter
   * in the fetch request, narrowing results to a specific prime type.
   */
  it("fetchPrimes applies form filter", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ primes: [], total: 0, limit: 50, offset: 0 }),
    });

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { form: "factorial" });
    });

    const calls = mockFetch.mock.calls;
    const lastCallUrl = calls[calls.length - 1][0] as string;
    expect(lastCallUrl).toContain("form=factorial");
  });

  /**
   * Verifies that the search filter is passed as a URL parameter
   * in the fetch request for expression text matching.
   */
  it("fetchPrimes applies search filter", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ primes: [], total: 0, limit: 50, offset: 0 }),
    });

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { search: "5!" });
    });

    const calls = mockFetch.mock.calls;
    const lastCallUrl = calls[calls.length - 1][0] as string;
    expect(lastCallUrl).toContain("search=5%21");
  });

  /**
   * Verifies that min_digits and max_digits filters are passed as URL
   * parameters for range-based filtering on the digits column.
   */
  it("fetchPrimes applies digit range filters", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ primes: [], total: 0, limit: 50, offset: 0 }),
    });

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { min_digits: 10, max_digits: 100 });
    });

    const calls = mockFetch.mock.calls;
    const lastCallUrl = calls[calls.length - 1][0] as string;
    expect(lastCallUrl).toContain("min_digits=10");
    expect(lastCallUrl).toContain("max_digits=100");
  });

  /** Verifies that clearSelectedPrime sets the selected prime to null. */
  it("clearSelectedPrime resets selection", async () => {
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ primes: [], total: 0, limit: 50, offset: 0 }),
    });

    const { result } = renderHook(() => usePrimes());

    act(() => result.current.clearSelectedPrime());
    expect(result.current.selectedPrime).toBeNull();
  });

  /**
   * Verifies that fetchPrimeDetail calls the correct detail endpoint
   * and sets the selectedPrime state.
   */
  it("fetchPrimeDetail fetches a single prime", async () => {
    const detailData = {
      id: 42,
      form: "factorial",
      expression: "100!+1",
      digits: 158,
      found_at: "2026-01-15T00:00:00Z",
      search_params: "{}",
      proof_method: "deterministic",
      verified: true,
      verified_at: "2026-01-15T01:00:00Z",
      verification_method: "BPSW",
      verification_tier: 2,
    };

    // First call is initial mount fetch, second is the detail fetch
    mockFetch
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ primes: [], total: 0, limit: 50, offset: 0 }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve(detailData),
      });

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimeDetail(42);
    });

    expect(result.current.selectedPrime).toEqual(detailData);
    const calls = mockFetch.mock.calls;
    const detailCallUrl = calls[calls.length - 1][0] as string;
    expect(detailCallUrl).toContain("/api/primes/42");
  });
});
