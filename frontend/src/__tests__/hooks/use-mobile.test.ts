/**
 * @file Tests for useIsMobile hook
 * @module __tests__/hooks/use-mobile
 *
 * Validates the mobile detection hook which uses window.innerWidth to
 * determine if the viewport is below the 768px mobile breakpoint.
 * This hook drives responsive layout decisions across the dashboard,
 * such as collapsing navigation, stacking cards, and adjusting chart sizes.
 *
 * The tests mock window.matchMedia and set window.innerWidth to simulate
 * different viewport sizes. The breakpoint boundary (768px) is explicitly
 * tested to ensure correct behavior at the exact threshold.
 *
 * @see {@link ../../hooks/use-mobile} Source hook
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useIsMobile } from "@/hooks/use-mobile";

// Tests mobile detection across different viewport widths.
// Uses window.innerWidth to simulate device sizes and validates
// the 768px breakpoint boundary.
describe("useIsMobile", () => {
  let addListenerCallback: (() => void) | null = null;

  beforeEach(() => {
    addListenerCallback = null;
    // Mock matchMedia to prevent errors in JSDOM environment.
    // The addEventListener capture allows tests to simulate resize events.
    vi.spyOn(window, "matchMedia").mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn((_event: string, cb: () => void) => {
        addListenerCallback = cb;
      }),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }));
  });

  /**
   * Verifies that a 1024px viewport (typical desktop) returns false.
   * This is well above the 768px breakpoint.
   */
  it("returns false for desktop width", () => {
    Object.defineProperty(window, "innerWidth", { value: 1024, writable: true });
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(false);
  });

  /**
   * Verifies that a 500px viewport (typical mobile) returns true.
   * This is well below the 768px breakpoint.
   */
  it("returns true for mobile width", () => {
    Object.defineProperty(window, "innerWidth", { value: 500, writable: true });
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(true);
  });

  /**
   * Verifies behavior at the exact breakpoint boundary (768px).
   * At 768px, the hook returns false (not mobile), meaning 768px
   * is considered a desktop-class width.
   */
  it("returns false at exactly 768px (breakpoint boundary)", () => {
    Object.defineProperty(window, "innerWidth", { value: 768, writable: true });
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(false);
  });

  /**
   * Verifies that 767px (one pixel below the breakpoint) returns true.
   * This confirms the breakpoint is exclusive: width < 768 is mobile.
   */
  it("returns true at 767px", () => {
    Object.defineProperty(window, "innerWidth", { value: 767, writable: true });
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(true);
  });
});
