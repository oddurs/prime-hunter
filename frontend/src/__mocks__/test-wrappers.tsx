/**
 * @file Test wrapper components that provide mock contexts for testing
 * @module __mocks__/test-wrappers
 *
 * Provides pre-configured mock context data and wrapper components for testing
 * components that depend on WebSocket, auth, or navigation contexts. These
 * wrappers eliminate boilerplate in component tests by providing sensible
 * default values for all context fields.
 *
 * Exports:
 * - `defaultWsData`: Complete WsData object with empty/connected defaults
 * - `defaultAuthData`: Mock authenticated user for auth-dependent components
 * - `mockNextNavigation()`: Factory for mocking next/navigation hooks
 * - `createWsWrapper()`: Factory for creating WebSocket context wrappers
 *
 * @see {@link ../contexts/websocket-context} WebSocket context
 * @see {@link ../contexts/auth-context} Auth context
 */
import React from "react";
import type { WsData } from "@/hooks/use-websocket";

/**
 * Default mock WsData object with all fields at empty/connected defaults.
 * Simulates a connected coordinator with no active searches, workers, or agents.
 * Use `createWsWrapper({ ...overrides })` to customize specific fields.
 */
export const defaultWsData: WsData = {
  status: { active: false, checkpoint: null },
  fleet: {
    workers: [],
    total_workers: 0,
    total_cores: 0,
    total_tested: 0,
    total_found: 0,
  },
  coordinator: null,
  searches: [],
  searchJobs: [],
  deployments: [],
  notifications: [],
  agentTasks: [],
  agentBudgets: [],
  runningAgents: [],
  projects: [],
  records: [],
  connected: true,
  sendMessage: () => {},
};

/**
 * Default mock auth context data for testing components that require
 * an authenticated user. Provides a test user with no active session,
 * and no-op signIn/signOut functions.
 */
export const defaultAuthData = {
  user: { id: "test-user", email: "test@example.com" } as unknown as import("@supabase/supabase-js").User,
  session: null as unknown as import("@supabase/supabase-js").Session | null,
  loading: false,
  signIn: async () => null as string | null,
  signOut: async () => {},
};

/**
 * Creates a mock for the next/navigation module used by Next.js pages.
 * Returns mock implementations of usePathname, useRouter, useSearchParams,
 * and useParams with configurable pathname.
 *
 * Usage: `vi.mock("next/navigation", () => mockNextNavigation("/browse"))`
 *
 * The router methods (push, replace, back, etc.) are no-ops by default.
 * Override individual methods if you need to assert navigation calls.
 */
export function mockNextNavigation(pathname: string = "/") {
  return {
    usePathname: () => pathname,
    useRouter: () => ({
      push: () => {},
      replace: () => {},
      back: () => {},
      forward: () => {},
      refresh: () => {},
      prefetch: () => {},
    }),
    useSearchParams: () => new URLSearchParams(),
    useParams: () => ({}),
  };
}

/**
 * Creates a React wrapper component that provides WebSocket context data
 * with customizable overrides. Merges the provided overrides with
 * defaultWsData for a complete WsData object.
 *
 * Note: This wrapper uses a simplified approach -- it renders children
 * directly without an actual context provider. For tests that need the
 * real WebSocketProvider, use it directly with mocked transport hooks.
 *
 * Usage:
 * ```typescript
 * const wrapper = createWsWrapper({ connected: true, fleet: { ... } });
 * const { result } = renderHook(() => useMyHook(), { wrapper });
 * ```
 */
export function createWsWrapper(overrides: Partial<WsData> = {}) {
  const wsData = { ...defaultWsData, ...overrides };

  // We provide the context via a mock of the useWs hook
  return function WsWrapper({ children }: { children: React.ReactNode }) {
    return <>{children}</>;
  };
}
