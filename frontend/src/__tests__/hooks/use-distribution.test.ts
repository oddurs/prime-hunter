import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useDistribution } from "@/hooks/use-distribution";

describe("useDistribution", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("fetches distribution on mount with default bucket size", async () => {
    const mockData = [
      { bucket_start: 0, form: "factorial", count: 10 },
      { bucket_start: 10, form: "factorial", count: 5 },
    ];
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(result.current.distribution).toHaveLength(2);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_digit_distribution", {
      bucket_size_param: 10,
    });
  });

  it("passes custom bucket size", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    renderHook(() => useDistribution(50));

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalledWith("get_digit_distribution", {
        bucket_size_param: 50,
      });
    });
  });

  it("handles empty response", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });

  it("returns empty on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useDistribution());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.distribution).toEqual([]);
  });
});
