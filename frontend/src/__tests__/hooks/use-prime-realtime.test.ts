/**
 * @file Tests for usePrimeRealtime hook
 * @module __tests__/hooks/use-prime-realtime
 *
 * Validates the WebSocket-based real-time prime discovery notification hook.
 * This hook reads `lastPrimeFound` from the WebSocket context and fires
 * a callback when a new prime is discovered. It surfaces the latest event
 * for toast notifications and live table updates.
 *
 * The hook consumes `useWs()` from the WebSocket context rather than
 * subscribing to Supabase Realtime directly.
 *
 * @see {@link ../../hooks/use-prime-realtime} Source hook
 * @see {@link ../../components/prime-notifier} Toast notification component
 * @see {@link ../../contexts/websocket-context} WebSocket context
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { defaultWsData } from "@/__mocks__/test-wrappers";
import type { WsData } from "@/hooks/use-websocket";

// Mock the WebSocket context to control lastPrimeFound
let mockWsData: WsData = { ...defaultWsData };

vi.mock("@/contexts/websocket-context", () => ({
  useWs: () => mockWsData,
}));

import { usePrimeRealtime } from "@/hooks/use-prime-realtime";

describe("usePrimeRealtime", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockWsData = { ...defaultWsData, lastPrimeFound: null };
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /** Verifies that newPrime is null when no prime has been found yet. */
  it("initially returns null", () => {
    const { result } = renderHook(() => usePrimeRealtime());
    expect(result.current.newPrime).toBeNull();
  });

  /**
   * Verifies that when lastPrimeFound is set in the WebSocket context,
   * the hook surfaces it as a RealtimePrime object.
   */
  it("returns newPrime when lastPrimeFound is available", () => {
    mockWsData = {
      ...defaultWsData,
      lastPrimeFound: {
        form: "factorial",
        expression: "5!+1",
        digits: 3,
        proof_method: "deterministic",
        timestamp_ms: 1706000000000,
      },
    };

    const { result } = renderHook(() => usePrimeRealtime());

    expect(result.current.newPrime).not.toBeNull();
    expect(result.current.newPrime!.form).toBe("factorial");
    expect(result.current.newPrime!.expression).toBe("5!+1");
    expect(result.current.newPrime!.digits).toBe(3);
    expect(result.current.newPrime!.id).toBe(0);
  });

  /**
   * Verifies that the onPrimeFound callback is fired when lastPrimeFound
   * changes from null to a prime object.
   */
  it("calls onPrimeFound callback when a prime is found", () => {
    const onPrimeFound = vi.fn();

    mockWsData = {
      ...defaultWsData,
      lastPrimeFound: {
        form: "kbn",
        expression: "3*2^100+1",
        digits: 31,
        proof_method: "Proth",
        timestamp_ms: 1706000000000,
      },
    };

    renderHook(() => usePrimeRealtime(onPrimeFound));

    expect(onPrimeFound).toHaveBeenCalledWith(
      expect.objectContaining({
        form: "kbn",
        expression: "3*2^100+1",
        digits: 31,
      })
    );
  });

  /**
   * Verifies that the onPrimeFound callback is not fired when lastPrimeFound
   * is null (no prime discovered yet).
   */
  it("does not call onPrimeFound when no prime is available", () => {
    const onPrimeFound = vi.fn();

    mockWsData = { ...defaultWsData, lastPrimeFound: null };

    renderHook(() => usePrimeRealtime(onPrimeFound));

    expect(onPrimeFound).not.toHaveBeenCalled();
  });
});
