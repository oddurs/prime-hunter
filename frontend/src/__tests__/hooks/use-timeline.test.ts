import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";

const mockRpc = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    rpc: (...args: unknown[]) => mockRpc(...args),
  },
}));

import { useTimeline } from "@/hooks/use-timeline";

describe("useTimeline", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("fetches timeline on mount with default bucket type", async () => {
    const mockData = [
      { bucket: "2026-01-01", form: "factorial", count: 5 },
      { bucket: "2026-01-02", form: "factorial", count: 3 },
    ];
    mockRpc.mockResolvedValue({ data: mockData, error: null });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(result.current.timeline).toHaveLength(2);
    });
    expect(mockRpc).toHaveBeenCalledWith("get_discovery_timeline", {
      bucket_type: "day",
    });
  });

  it("passes custom bucket type", async () => {
    mockRpc.mockResolvedValue({ data: [], error: null });

    renderHook(() => useTimeline("hour"));

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalledWith("get_discovery_timeline", {
        bucket_type: "hour",
      });
    });
  });

  it("returns empty array on error", async () => {
    mockRpc.mockResolvedValue({ data: null, error: { message: "fail" } });

    const { result } = renderHook(() => useTimeline());

    await waitFor(() => {
      expect(mockRpc).toHaveBeenCalled();
    });
    expect(result.current.timeline).toEqual([]);
  });
});
