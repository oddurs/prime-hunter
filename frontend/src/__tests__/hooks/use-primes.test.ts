import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";

// Mock supabase before importing the hook
const mockSelect = vi.fn();
const mockEq = vi.fn();
const mockIlike = vi.fn();
const mockGte = vi.fn();
const mockLte = vi.fn();
const mockOrder = vi.fn();
const mockRange = vi.fn();
const mockSingle = vi.fn();
const mockFrom = vi.fn();

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

describe("usePrimes", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

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

  it("returns empty data on error", async () => {
    setupChain(null, null, { message: "Network error" });

    const { result } = renderHook(() => usePrimes());

    // Should not throw, just keep empty state
    await waitFor(() => {
      expect(mockFrom).toHaveBeenCalled();
    });
    expect(result.current.primes.primes).toHaveLength(0);
  });

  it("fetchPrimes applies form filter", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { form: "factorial" });
    });

    expect(mockEq).toHaveBeenCalledWith("form", "factorial");
  });

  it("fetchPrimes applies search filter", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { search: "5!" });
    });

    expect(mockIlike).toHaveBeenCalledWith("expression", "%5!%");
  });

  it("fetchPrimes applies digit range filters", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    await act(async () => {
      await result.current.fetchPrimes(0, 50, { min_digits: 10, max_digits: 100 });
    });

    expect(mockGte).toHaveBeenCalledWith("digits", 10);
    expect(mockLte).toHaveBeenCalledWith("digits", 100);
  });

  it("clearSelectedPrime resets selection", async () => {
    setupChain([], 0, null);

    const { result } = renderHook(() => usePrimes());

    act(() => result.current.clearSelectedPrime());
    expect(result.current.selectedPrime).toBeNull();
  });
});
