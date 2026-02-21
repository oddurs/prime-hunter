/**
 * @file Tests for useTheme hook
 * @module __tests__/hooks/use-theme
 *
 * Validates the theme management hook. The app is dark-only, so the hook
 * always returns "dark" and toggle/set are no-ops.
 *
 * @see {@link ../../hooks/use-theme} Source hook
 */
import { describe, it, expect, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useTheme } from "@/hooks/use-theme";

describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  /** Verifies that the hook always returns dark theme. */
  it("defaults to dark theme", () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("dark");
  });

  /** Verifies that the hook returns dark even with a stored light preference. */
  it("ignores stored light theme", () => {
    localStorage.setItem("darkreach-theme", "light");
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("dark");
  });

  /** Verifies toggle is a no-op — always dark. */
  it("toggleTheme is a no-op", () => {
    const { result } = renderHook(() => useTheme());
    result.current.toggleTheme();
    expect(result.current.theme).toBe("dark");
  });
});
