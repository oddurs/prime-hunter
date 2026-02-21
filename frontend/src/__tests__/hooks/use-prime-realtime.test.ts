/**
 * @file Tests for usePrimeRealtime hook
 * @module __tests__/hooks/use-prime-realtime
 *
 * Validates the Supabase Realtime subscription hook for live prime discovery
 * notifications. This hook subscribes to INSERT events on the "primes" table
 * via Supabase's postgres_changes feature and exposes the most recently
 * discovered prime through the `newPrime` state.
 *
 * The hook is consumed by the PrimeNotifier component which displays toast
 * notifications when new primes are found during active searches.
 *
 * @see {@link ../../hooks/use-prime-realtime} Source hook
 * @see {@link ../../components/prime-notifier} Toast notification component
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";

// Mock the Supabase Realtime channel API
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

// Tests the Realtime subscription lifecycle: subscribe -> receive -> cleanup.
// Validates channel creation, event filtering, initial state, and unmount cleanup.
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

  /**
   * Verifies that the hook creates a "primes-inserts" channel and subscribes
   * to postgres_changes INSERT events on the public.primes table. Only INSERT
   * events are subscribed (not UPDATE or DELETE) since we only care about
   * newly discovered primes.
   */
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

  /** Verifies that newPrime is null before any INSERT event is received. */
  it("initially returns null", () => {
    const { result } = renderHook(() => usePrimeRealtime());
    expect(result.current.newPrime).toBeNull();
  });

  /**
   * Verifies that the Realtime channel is properly removed on unmount,
   * passing the exact channel mock object to removeChannel(). This
   * prevents memory leaks from orphaned subscriptions.
   */
  it("removes channel on unmount", () => {
    const { unmount } = renderHook(() => usePrimeRealtime());
    unmount();
    expect(mockRemoveChannel).toHaveBeenCalledWith(channelMock);
  });
});
