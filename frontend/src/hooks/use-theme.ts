"use client";

/**
 * @module use-theme
 *
 * Dark/light theme toggle hook. Persists the user's preference to
 * `localStorage` and applies it by toggling the `dark` class on the
 * document root element (Tailwind's dark mode strategy).
 */

import { useEffect, useState, useCallback } from "react";

type Theme = "light" | "dark";

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    if (typeof window === "undefined") return "dark";
    const stored = localStorage.getItem("darkreach-theme");
    return stored === "light" || stored === "dark" ? stored : "dark";
  });

  const setTheme = useCallback((t: Theme) => {
    setThemeState(t);
  }, []);

  useEffect(() => {
    document.documentElement.className = theme;
    localStorage.setItem("darkreach-theme", theme);
  }, [theme]);

  const toggleTheme = useCallback(() => {
    setTheme(theme === "dark" ? "light" : "dark");
  }, [theme, setTheme]);

  return { theme, setTheme, toggleTheme };
}
