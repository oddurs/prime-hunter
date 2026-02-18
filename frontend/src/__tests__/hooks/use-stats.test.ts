import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useStats } from "@/hooks/use-stats";

describe("useStats", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("fetches stats on mount", async () => {
    const mockData = {
      total: 42,
      by_form: [{ form: "factorial", count: 30 }],
      largest_digits: 1000,
      largest_expression: "100!+1",
    };
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).toEqual(mockData);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_stats");
  });

  it("returns null stats on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.stats).toBeNull();
  });

  it("provides a refetch function", async () => {
    mockRpc.mockResolvedValue({ data: { total: 1, by_form: [], largest_digits: 0, largest_expression: null }, error: null });

    const { result } = renderHook(() => useStats());

    await waitFor(() => {
      expect(result.current.stats).not.toBeNull();
    });
    expect(typeof result.current.refetch).toBe("function");
  });
});
