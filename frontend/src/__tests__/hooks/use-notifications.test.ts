/**
 * @file Tests for useBrowserNotifications hook
 * @module __tests__/hooks/use-notifications
 *
 * Validates the browser notification permission management hook which controls
 * desktop push notifications for prime discoveries. Tests cover the full
 * permission lifecycle: API detection, permission state reading, permission
 * requesting, enable/disable toggling, and localStorage persistence.
 *
 * The hook uses the browser Notification API and persists the enabled state
 * in localStorage under the key "darkreach-notifications-enabled". Permission
 * must be "granted" for notifications to be enabled; if denied, the hook
 * disables notifications regardless of the stored preference.
 *
 * @see {@link ../../hooks/use-notifications} Source hook
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useBrowserNotifications } from "@/hooks/use-notifications";

// Tests the browser notification permission lifecycle:
// detection -> initial state -> request permission -> enable/disable -> persist.
describe("useBrowserNotifications", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  /**
   * Verifies behavior when the Notification API is not available (e.g.,
   * in a server-side rendering context or unsupported browser). The hook
   * should report supported=false with default permission and disabled state.
   */
  it("detects when Notification API is not supported", async () => {
    // Remove Notification from window
    const original = window.Notification;
    // @ts-expect-error — removing Notification for test
    delete window.Notification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(false);
    });
    expect(result.current.permission).toBe("default");
    expect(result.current.enabled).toBe(false);

    // Restore
    window.Notification = original;
  });

  /**
   * Verifies that the hook reads the initial Notification.permission state
   * when the API is supported. With "default" permission, notifications
   * should be disabled until the user explicitly grants permission.
   */
  it("reads initial permission state when supported", async () => {
    // Mock Notification as supported with default permission
    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "default", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", { value: vi.fn(), configurable: true });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(true);
    });
    expect(result.current.permission).toBe("default");
    expect(result.current.enabled).toBe(false);
  });

  /**
   * Verifies that the hook restores the enabled state from localStorage
   * when the browser permission is already "granted". This allows
   * notifications to persist across page reloads without re-prompting.
   */
  it("restores enabled state from localStorage when permission is granted", async () => {
    localStorage.setItem("darkreach-notifications-enabled", "true");

    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "granted", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", {
      value: vi.fn().mockResolvedValue("granted"),
      configurable: true,
    });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(true);
    });
    expect(result.current.enabled).toBe(true);
    expect(result.current.permission).toBe("granted");
  });

  /**
   * Verifies that stored "enabled=true" is ignored when the browser
   * permission is "denied". This prevents stale localStorage values from
   * incorrectly showing notifications as enabled.
   */
  it("does not restore enabled state when permission is not granted", async () => {
    localStorage.setItem("darkreach-notifications-enabled", "true");

    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "denied", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", { value: vi.fn(), configurable: true });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(true);
    });
    expect(result.current.enabled).toBe(false);
  });

  /**
   * Verifies that setEnabled(true) triggers a Notification.requestPermission()
   * call when the current permission is "default" (not yet decided).
   * On successful grant, the enabled state is persisted to localStorage.
   */
  it("requests permission when enabling and permission is default", async () => {
    const mockRequestPermission = vi.fn().mockResolvedValue("granted");
    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "default", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", {
      value: mockRequestPermission,
      configurable: true,
    });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(true);
    });

    await act(async () => {
      await result.current.setEnabled(true);
    });

    expect(mockRequestPermission).toHaveBeenCalled();
    expect(localStorage.getItem("darkreach-notifications-enabled")).toBe("true");
  });

  /**
   * Verifies that notifications remain disabled when the user denies
   * the permission prompt. The localStorage value is set to "false"
   * to prevent future re-prompting.
   */
  it("does not enable when permission request is denied", async () => {
    const mockRequestPermission = vi.fn().mockResolvedValue("denied");
    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "default", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", {
      value: mockRequestPermission,
      configurable: true,
    });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.supported).toBe(true);
    });

    await act(async () => {
      await result.current.setEnabled(true);
    });

    expect(result.current.enabled).toBe(false);
    expect(localStorage.getItem("darkreach-notifications-enabled")).toBe("false");
  });

  /**
   * Verifies that calling setEnabled(false) disables notifications
   * and persists "false" to localStorage. This allows the user to
   * opt out even when browser permission is granted.
   */
  it("disables notifications and updates localStorage", async () => {
    localStorage.setItem("darkreach-notifications-enabled", "true");

    const mockNotification = vi.fn() as unknown as typeof Notification;
    Object.defineProperty(mockNotification, "permission", { value: "granted", configurable: true });
    Object.defineProperty(mockNotification, "requestPermission", { value: vi.fn(), configurable: true });
    window.Notification = mockNotification;

    const { result } = renderHook(() => useBrowserNotifications());

    await waitFor(() => {
      expect(result.current.enabled).toBe(true);
    });

    await act(async () => {
      await result.current.setEnabled(false);
    });

    expect(result.current.enabled).toBe(false);
    expect(localStorage.getItem("darkreach-notifications-enabled")).toBe("false");
  });
});
