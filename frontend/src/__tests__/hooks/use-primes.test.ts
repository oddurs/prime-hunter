/**
 * @file Tests for usePrimes hook
 * @module __tests__/hooks/use-primes
 *
 * Validates the prime records data hook which provides paginated, filterable
 * access to the primes table via Supabase. This is the primary data hook for
 * the browse page and primes table component, supporting filters by form type,
 * expression search (ILIKE), digit range (gte/lte), and paginated range queries.
 *
 * The mock chain supports the full query pattern:
 * from("primes").select("*", { count: "exact" }).eq().ilike().gte().lte().order().range()
 *
 * @see {@link ../../hooks/use-primes} Source hook
 * @see {@link ../../components/primes-table} Primes table component
 * @see {@link ../../app/browse/page} Browse page
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// Mock supabase before importing the hook.
// Includes filter methods (eq, ilike, gte, lte) and range-based pagination.
const mockSelect = vi.fn();
const mockEq = vi.fn();
const mockIlike = vi.fn();
const mockGte = vi.fn();
const mockLte = vi.fn();
const mockOrder = vi.fn();
const mockRange = vi.fn();
const mockSingle = vi.fn();
const mockFrom = vi.fn();

/**
 * Configures the mock query chain for paginated queries.
 * The chain resolves at .range() with data, count, and error fields.
 * Count is used for displaying total results and pagination controls.
 */
function setupChain(finalData: unknown, finalCount: number | null, finalError: unknown) {
  const chain = {
    select: mockSelect.mockReturnThis(),
    eq: mockEq.mockReturnThis(),
    ilike: mockIlike.mockReturnThis(),
    gte: mockGte.mockReturnThis(),
    lte: mockLte.mockReturnThis(),
    order: mockOrder.mockReturnThis(),
    range: mockRange.mockResolvedValue({
      data: finalData,
      count: finalCount,
      error: finalError,
    }),
    single: mockSingle.mockResolvedValue({
      data: finalData,
      error: finalError,
    }),
  };
  mockFrom.mockReturnValue(chain);
  return chain;
}

vi.mock("@/lib/supabase", () => ({
  supabase: {
    from: (...args: unknown[]) => mockFrom(...args),
  },
}));

import { usePrimes } from "@/hooks/use-primes";

// Tests the prime data fetching and filtering lifecycle.
// Validates initial fetch, filter application, and selection state.
describe("usePrimes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that the hook fetches primes from the "primes" table on mount.
   * The mock returns one factorial prime (5!+1 = 121, 3 digits).
   */
  it("fetches primes on mount", async () => {
    const mockData = [
      { id: 1, form: "factorial", expression: "5!+1", digits: 3, found_at: "2026-01-01T00:00:00Z", proof_method: "deterministic", verified: false, verified_at: null, verification_method: null, verification_tier: null },
    ];
    setupChain(mockData, 1, null);

    const { result } = renderHook(() => usePrimes());

    await waitFor(() => {
      expect(result.current.primes.primes).toHaveLength(1);
    });
    expect(result.current.primes.total).toBe(1);
    expect(mockFrom).toHaveBeenCalledWith("primes");
  });

  /**
   * Verifies graceful error handling; primes array stays empty and no
   * exception is thrown to the consuming component.
   */
  it("returns empty data on error", async () => {
    setupChain(null, null, { message: "Network error" });

    const { result } = renderHook(() => usePrimes());

    // Should not throw, just keep empty state
    await waitFor(() => {
      expect(mockFrom).toHaveBeenCalled();
    });
    expect(result.current.primes.primes).toHaveLength(0);
  });

  /**
   * Verifies that the form filter adds an .eq("form", "factorial") clause
   * to the Supabase query, narrowing results to a specific prime type.
   */
  it("fetchPrimes applies form filter", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { form: "factorial" });
    });

    expect(mockEq).toHaveBeenCalledWith("form", "factorial");
  });

  /**
   * Verifies that the search filter uses .ilike("expression", "%5!%")
   * for case-insensitive pattern matching on the expression column.
   */
  it("fetchPrimes applies search filter", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { search: "5!" });
    });

    expect(mockIlike).toHaveBeenCalledWith("expression", "%5!%");
  });

  /**
   * Verifies that min_digits and max_digits filters apply .gte() and .lte()
   * constraints on the digits column for range-based filtering.
   */
  it("fetchPrimes applies digit range filters", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { min_digits: 10, max_digits: 100 });
    });

    expect(mockGte).toHaveBeenCalledWith("digits", 10);
    expect(mockLte).toHaveBeenCalledWith("digits", 100);
  });

  /** Verifies that clearSelectedPrime sets the selected prime to null. */
  it("clearSelectedPrime resets selection", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    act(() => result.current.clearSelectedPrime());
    expect(result.current.selectedPrime).toBeNull();
  });
});
