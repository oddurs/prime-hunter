/**
 * @file Tests for AuthProvider, useAuth, AuthGuard, and RoleGuard
 * @module __tests__/contexts/auth-context
 *
 * Comprehensive test suite for the authentication context which provides
 * Supabase Auth integration across the dashboard. Tests cover:
 *
 * - AuthProvider: Session initialization, user state management, auth state
 *   change listener, profile fetching (role + operator_id), sign in/out
 * - useAuth: Context hook with error when used outside provider
 * - AuthGuard: Component that shows loading, hides content when unauthenticated,
 *   and renders children when authenticated
 * - RoleGuard: Component that enforces role-based access control (e.g., admin-only)
 *
 * The mock setup covers both Supabase Auth methods (getSession, onAuthStateChange,
 * signInWithPassword, signOut) and the profile query chain (from().select().eq().single()).
 *
 * @see {@link ../../contexts/auth-context} Source context
 * @see {@link ../../app/layout} Root layout using AuthProvider
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { render, screen } from "@testing-library/react";
import React from "react";

// --- Supabase auth mock ---
// Mocks both the auth API methods and the profile table query chain.
const mockGetSession = vi.fn();
const mockOnAuthStateChange = vi.fn();
const mockSignInWithPassword = vi.fn();
const mockSignOut = vi.fn();
const mockFromSelect = vi.fn();
const mockFromEq = vi.fn();
const mockFromSingle = vi.fn();
const mockFrom = vi.fn();

vi.mock("@/lib/supabase", () => ({
  supabase: {
    auth: {
      getSession: (...args: unknown[]) => mockGetSession(...args),
      onAuthStateChange: (...args: unknown[]) => mockOnAuthStateChange(...args),
      signInWithPassword: (...args: unknown[]) => mockSignInWithPassword(...args),
      signOut: (...args: unknown[]) => mockSignOut(...args),
    },
    from: (...args: unknown[]) => mockFrom(...args),
  },
}));

import { AuthProvider, useAuth, AuthGuard, RoleGuard } from "@/contexts/auth-context";

/**
 * Sets up the default auth mocks for getSession and onAuthStateChange.
 * When session is null, simulates an unauthenticated state.
 */
function setupDefaultAuthMocks(session: unknown = null) {
  mockGetSession.mockResolvedValue({
    data: { session },
  });
  mockOnAuthStateChange.mockReturnValue({
    data: { subscription: { unsubscribe: vi.fn() } },
  });
}

/**
 * Sets up the profile query mock for from("profiles").select().eq("id", userId).single().
 * Profile data includes role and operator_id for RBAC.
 */
function setupProfileMock(profileData: unknown = null, error: unknown = null) {
  const chain = {
    select: mockFromSelect.mockReturnThis(),
    eq: mockFromEq.mockReturnThis(),
    single: mockFromSingle.mockResolvedValue({ data: profileData, error }),
  };
  mockFrom.mockReturnValue(chain);
}

/** Test wrapper component that wraps children in AuthProvider. */
function AuthProviderWrapper({ children }: { children: React.ReactNode }) {
  return <AuthProvider>{children}</AuthProvider>;
}

// Tests the AuthProvider context which manages the full auth lifecycle:
// session check on mount, user state, profile fetching, auth event handling.
describe("AuthProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies the initial loading state before getSession completes.
   * The provider should start with loading=true and user=null.
   */
  it("starts with loading true and no user", async () => {
    setupDefaultAuthMocks(null);

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    // Initially loading
    expect(result.current.loading).toBe(true);
    expect(result.current.user).toBeNull();

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
  });

  /**
   * Verifies that an existing Supabase session is detected on mount,
   * the user is set, and the profile is fetched to determine role
   * and operator_id.
   */
  it("sets user from existing session", async () => {
    const session = {
      user: { id: "user-123", email: "test@example.com" },
      access_token: "token",
    };
    setupDefaultAuthMocks(session);
    setupProfileMock({ role: "admin", operator_id: "op-1" });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.user?.id).toBe("user-123");
    expect(result.current.role).toBe("admin");
    expect(result.current.operatorId).toBe("op-1");
  });

  /**
   * Verifies that the role defaults to "operator" when the profile
   * query fails (e.g., new user without a profile row). This is the
   * least-privileged default role.
   */
  it("defaults to operator role when no profile exists", async () => {
    const session = {
      user: { id: "user-456", email: "new@example.com" },
      access_token: "token",
    };
    setupDefaultAuthMocks(session);
    setupProfileMock(null, { message: "Not found" });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.role).toBe("operator");
    expect(result.current.operatorId).toBeNull();
  });

  /**
   * Verifies successful sign-in via email/password. The function should
   * call signInWithPassword and return null (no error).
   */
  it("handles signIn successfully", async () => {
    setupDefaultAuthMocks(null);
    mockSignInWithPassword.mockResolvedValue({ error: null });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    let error: string | null = null;
    await act(async () => {
      error = await result.current.signIn("test@example.com", "password123");
    });

    expect(error).toBeNull();
    expect(mockSignInWithPassword).toHaveBeenCalledWith({
      email: "test@example.com",
      password: "password123",
    });
  });

  /**
   * Verifies that sign-in failure returns the error message string
   * rather than throwing. The consuming login form displays this message.
   */
  it("returns error message on signIn failure", async () => {
    setupDefaultAuthMocks(null);
    mockSignInWithPassword.mockResolvedValue({
      error: { message: "Invalid credentials" },
    });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    let error: string | null = null;
    await act(async () => {
      error = await result.current.signIn("bad@example.com", "wrong");
    });

    expect(error).toBe("Invalid credentials");
  });

  /** Verifies that signOut calls supabase.auth.signOut(). */
  it("handles signOut", async () => {
    setupDefaultAuthMocks(null);
    mockSignOut.mockResolvedValue({});

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    await act(async () => {
      await result.current.signOut();
    });

    expect(mockSignOut).toHaveBeenCalled();
  });

  /**
   * Verifies that the provider reacts to Supabase auth state changes.
   * When onAuthStateChange fires with SIGNED_IN, the user and profile
   * should be updated accordingly.
   */
  it("reacts to onAuthStateChange", async () => {
    let authChangeCallback: (event: string, session: unknown) => void = () => {};
    mockGetSession.mockResolvedValue({ data: { session: null } });
    mockOnAuthStateChange.mockImplementation((cb: (event: string, session: unknown) => void) => {
      authChangeCallback = cb;
      return { data: { subscription: { unsubscribe: vi.fn() } } };
    });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.user).toBeNull();

    // Simulate auth state change (user logs in)
    setupProfileMock({ role: "admin", operator_id: null });

    act(() => {
      authChangeCallback("SIGNED_IN", {
        user: { id: "new-user", email: "new@test.com" },
        access_token: "t",
      });
    });

    await waitFor(() => {
      expect(result.current.user?.id).toBe("new-user");
    });
  });

  /**
   * Verifies that SIGNED_OUT auth events clear the user, role, and
   * operatorId state. This ensures the UI reverts to the login screen.
   */
  it("clears role and operatorId on sign out event", async () => {
    let authChangeCallback: (event: string, session: unknown) => void = () => {};
    const session = {
      user: { id: "user-1", email: "u@e.com" },
      access_token: "t",
    };
    mockGetSession.mockResolvedValue({ data: { session } });
    mockOnAuthStateChange.mockImplementation((cb: (event: string, session: unknown) => void) => {
      authChangeCallback = cb;
      return { data: { subscription: { unsubscribe: vi.fn() } } };
    });
    setupProfileMock({ role: "admin", operator_id: "op-1" });

    const { result } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(result.current.role).toBe("admin");
    });

    // Simulate sign out
    act(() => {
      authChangeCallback("SIGNED_OUT", null);
    });

    await waitFor(() => {
      expect(result.current.user).toBeNull();
      expect(result.current.role).toBeNull();
      expect(result.current.operatorId).toBeNull();
    });
  });

  /**
   * Verifies that the onAuthStateChange subscription is cleaned up on
   * provider unmount, preventing memory leaks and stale callbacks.
   */
  it("unsubscribes from auth changes on unmount", async () => {
    const unsubscribe = vi.fn();
    mockGetSession.mockResolvedValue({ data: { session: null } });
    mockOnAuthStateChange.mockReturnValue({
      data: { subscription: { unsubscribe } },
    });

    const { unmount } = renderHook(() => useAuth(), {
      wrapper: AuthProviderWrapper,
    });

    await waitFor(() => {
      expect(mockOnAuthStateChange).toHaveBeenCalled();
    });

    unmount();
    expect(unsubscribe).toHaveBeenCalled();
  });
});

// Tests the useAuth hook's error boundary when used outside AuthProvider.
describe("useAuth outside provider", () => {
  /**
   * Verifies that useAuth throws a descriptive error when called
   * outside of an AuthProvider. This prevents silent null context bugs.
   */
  it("throws when used outside AuthProvider", () => {
    // Suppress console.error for expected error
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(() => {
      renderHook(() => useAuth());
    }).toThrow("useAuth must be used within an AuthProvider");

    consoleSpy.mockRestore();
  });
});

// Tests the AuthGuard component which gates access to authenticated content.
// Shows loading state during auth check, renders nothing for unauthenticated
// users, and renders children for authenticated users.
describe("AuthGuard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that AuthGuard shows "Loading..." text while the auth
   * session is being checked. The mock never resolves getSession to
   * keep the loading state permanently true.
   */
  it("shows loading state while auth is loading", async () => {
    // Never resolve getSession to keep loading=true
    mockGetSession.mockReturnValue(new Promise(() => {}));
    mockOnAuthStateChange.mockReturnValue({
      data: { subscription: { unsubscribe: vi.fn() } },
    });

    render(
      <AuthProvider>
        <AuthGuard>
          <div>Protected content</div>
        </AuthGuard>
      </AuthProvider>
    );

    expect(screen.getByText("Loading...")).toBeDefined();
    expect(screen.queryByText("Protected content")).toBeNull();
  });

  /**
   * Verifies that AuthGuard renders nothing (empty container) when
   * there is no authenticated user. The protected content should not
   * be visible.
   */
  it("renders nothing when user is not authenticated", async () => {
    setupDefaultAuthMocks(null);

    const { container } = render(
      <AuthProvider>
        <AuthGuard>
          <div>Protected content</div>
        </AuthGuard>
      </AuthProvider>
    );

    await waitFor(() => {
      expect(screen.queryByText("Loading...")).toBeNull();
    });

    expect(screen.queryByText("Protected content")).toBeNull();
    // AuthGuard returns null, so container should be mostly empty
    expect(container.textContent).toBe("");
  });

  /**
   * Verifies that AuthGuard renders its children when the user is
   * authenticated with a valid session.
   */
  it("renders children when user is authenticated", async () => {
    const session = {
      user: { id: "user-1", email: "test@example.com" },
      access_token: "token",
    };
    setupDefaultAuthMocks(session);
    setupProfileMock({ role: "admin", operator_id: null });

    render(
      <AuthProvider>
        <AuthGuard>
          <div>Protected content</div>
        </AuthGuard>
      </AuthProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Protected content")).toBeDefined();
    });
  });
});

// Tests the RoleGuard component which enforces role-based access control.
// Shows "Access Denied" for insufficient roles and renders children
// for users with the required role.
describe("RoleGuard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  /**
   * Verifies that RoleGuard shows "Access Denied" when the user's role
   * (operator) does not match the required role (admin). The protected
   * content should not be rendered.
   */
  it("shows access denied for wrong role", async () => {
    const session = {
      user: { id: "user-1", email: "test@example.com" },
      access_token: "token",
    };
    setupDefaultAuthMocks(session);
    setupProfileMock({ role: "operator", operator_id: null });

    render(
      <AuthProvider>
        <RoleGuard requiredRole="admin">
          <div>Admin only</div>
        </RoleGuard>
      </AuthProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Access Denied")).toBeDefined();
    });
    expect(screen.queryByText("Admin only")).toBeNull();
  });

  /**
   * Verifies that RoleGuard renders its children when the user has
   * the required role (admin matches admin).
   */
  it("renders children for correct role", async () => {
    const session = {
      user: { id: "user-1", email: "test@example.com" },
      access_token: "token",
    };
    setupDefaultAuthMocks(session);
    setupProfileMock({ role: "admin", operator_id: null });

    render(
      <AuthProvider>
        <RoleGuard requiredRole="admin">
          <div>Admin only</div>
        </RoleGuard>
      </AuthProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Admin only")).toBeDefined();
    });
  });
});
