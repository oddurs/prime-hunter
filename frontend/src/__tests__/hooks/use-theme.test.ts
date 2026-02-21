/**
 * @file Tests for useTheme hook
 * @module __tests__/hooks/use-theme
 *
 * Validates the theme management hook which controls dark/light mode toggling
 * across the dashboard. The hook persists the theme preference in localStorage
 * under the key "darkreach-theme" and applies the theme class to
 * document.documentElement for CSS variable switching.
 *
 * The default theme is "dark" (matching the prime-hunting aesthetic).
 * Invalid stored values are treated as the default dark theme.
 *
 * @see {@link ../../hooks/use-theme} Source hook
 */
import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useTheme } from "@/hooks/use-theme";

// Tests the theme lifecycle: default -> toggle -> persist -> restore -> invalid values.
describe("useTheme", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.className = "";
  });

  /** Verifies that the hook defaults to dark theme when no preference is stored. */
  it("defaults to dark theme", () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("dark");
  });

  /**
   * Verifies that the hook reads and applies a previously stored theme
   * from localStorage on mount.
   */
  it("reads stored theme from localStorage", () => {
    localStorage.setItem("darkreach-theme", "light");
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("light");
  });

  /**
   * Verifies the toggle cycle: dark -> light -> dark. The toggleTheme
   * function alternates between the two themes with each call.
   */
  it("toggles between dark and light", () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("dark");

    act(() => result.current.toggleTheme());
    expect(result.current.theme).toBe("light");

    act(() => result.current.toggleTheme());
    expect(result.current.theme).toBe("dark");
  });

  /**
   * Verifies that setTheme writes the new value to localStorage under
   * the "darkreach-theme" key for cross-session persistence.
   */
  it("persists theme to localStorage", () => {
    const { result } = renderHook(() => useTheme());
    act(() => result.current.setTheme("light"));
    expect(localStorage.getItem("darkreach-theme")).toBe("light");
  });

  /**
   * Verifies that the hook applies the theme as a CSS class on
   * document.documentElement. This drives Tailwind's dark mode
   * via the "class" strategy (dark: prefix).
   */
  it("sets className on document.documentElement", () => {
    const { result } = renderHook(() => useTheme());
    expect(document.documentElement.className).toBe("dark");

    act(() => result.current.setTheme("light"));
    expect(document.documentElement.className).toBe("light");
  });

  /**
   * Verifies that invalid localStorage values (e.g., "invalid") are
   * treated as the default dark theme rather than causing errors.
   * This guards against corrupted or manually edited localStorage.
   */
  it("ignores invalid stored values", () => {
    localStorage.setItem("darkreach-theme", "invalid");
    const { result } = renderHook(() => useTheme());
    expect(result.current.theme).toBe("dark");
  });
});
