import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

const mockOn = vi.fn();
const mockSubscribe = vi.fn();
const mockChannel = vi.fn();
const mockRemoveChannel = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    channel: (...args: unknown[]) => mockChannel(...args),
    removeChannel: (...args: unknown[]) => mockRemoveChannel(...args),
  },
}));

import { usePrimeRealtime } from "@/hooks/use-prime-realtime";

describe("usePrimeRealtime", () => {
  let channelMock: Record<string, unknown>;

  beforeEach(() => {
    vi.clearAllMocks();
    channelMock = {
      on: mockOn,
      subscribe: mockSubscribe.mockReturnThis(),
    };
    mockOn.mockReturnValue(channelMock);
    mockChannel.mockReturnValue(channelMock);
  });

  it("subscribes to primes-inserts channel on mount", () => {
    renderHook(() => usePrimeRealtime());

    expect(mockChannel).toHaveBeenCalledWith("primes-inserts");
    expect(mockOn).toHaveBeenCalledWith(
      "postgres_changes",
      { event: "INSERT", schema: "public", table: "primes" },
      expect.any(Function)
    );
    expect(mockSubscribe).toHaveBeenCalled();
  });

  it("initially returns null", () => {
    const { result } = renderHook(() => usePrimeRealtime());
    expect(result.current.newPrime).toBeNull();
  });

  it("removes channel on unmount", () => {
    const { unmount } = renderHook(() => usePrimeRealtime());
    unmount();
    expect(mockRemoveChannel).toHaveBeenCalledWith(channelMock);
  });
});
