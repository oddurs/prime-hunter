"use client";

/**
 * @module use-theme
 *
 * Theme hook — dark mode only. Exported API is preserved for backward
 * compatibility but toggleTheme and setTheme are no-ops.
 */

type Theme = "dark";

export function useTheme() {
  const theme: Theme = "dark";
  const setTheme = () => {};
  const toggleTheme = () => {};
  return { theme, setTheme, toggleTheme };
}
