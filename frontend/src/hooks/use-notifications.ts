"use client";

/**
 * @module use-notifications
 *
 * React hook for Browser Notification API integration.
 *
 * Manages the permission lifecycle (default → granted/denied), persists
 * the user's opt-in preference to `localStorage`, and provides a
 * `notify()` function for showing system-level notifications when the
 * browser tab is unfocused (e.g., when a new prime is discovered).
 */

import { useEffect, useState, useCallback } from "react";

const STORAGE_KEY = "darkreach-notifications-enabled";

/**
 * Hook for Browser Notification API integration.
 *
 * Manages permission state and localStorage persistence for
 * opt-in browser notifications when the tab is unfocused.
 */
export function useBrowserNotifications() {
  const [supported, setSupported] = useState(false);
  const [permission, setPermission] = useState<NotificationPermission>("default");
  const [enabled, setEnabledState] = useState(false);

  // SSR-safe initialization
  useEffect(() => {
    if (typeof window === "undefined") return;
    const isSupported = "Notification" in window;
    setSupported(isSupported);
    if (isSupported) {
      setPermission(Notification.permission);
      const stored = localStorage.getItem(STORAGE_KEY);
      // Only enable if previously opted in AND permission is still granted
      if (stored === "true" && Notification.permission === "granted") {
        setEnabledState(true);
      }
    }
  }, []);

  const setEnabled = useCallback(async (value: boolean) => {
    if (!supported) return;

    if (value) {
      // Request permission lazily on first enable
      if (Notification.permission === "default") {
        const result = await Notification.requestPermission();
        setPermission(result);
        if (result !== "granted") {
          localStorage.setItem(STORAGE_KEY, "false");
          setEnabledState(false);
          return;
        }
      } else if (Notification.permission === "denied") {
        // Can't enable if blocked
        return;
      }
      localStorage.setItem(STORAGE_KEY, "true");
      setEnabledState(true);
    } else {
      localStorage.setItem(STORAGE_KEY, "false");
      setEnabledState(false);
    }
  }, [supported]);

  const show = useCallback((title: string, options?: NotificationOptions) => {
    if (!supported || !enabled) return;
    if (Notification.permission !== "granted") return;
    // Only show when tab is not focused
    if (document.hasFocus()) return;

    try {
      new Notification(title, {
        icon: "/icon-192.png",
        ...options,
      });
    } catch {
      // Notification constructor can throw in some environments
    }
  }, [supported, enabled]);

  return { supported, permission, enabled, setEnabled, show };
}
