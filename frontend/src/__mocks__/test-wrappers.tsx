/**
 * Test wrapper components that provide mock contexts.
 */
import React from "react";
import type { WsData } from "@/hooks/use-websocket";

// Default mock WsData
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

// Default mock auth context
export const defaultAuthData = {
  user: { id: "test-user", email: "test@example.com" } as unknown as import("@supabase/supabase-js").User,
  session: null as unknown as import("@supabase/supabase-js").Session | null,
  loading: false,
  signIn: async () => null as string | null,
  signOut: async () => {},
};

/**
 * Mock next/navigation for tests.
 * Usage: vi.mock("next/navigation", () => mockNextNavigation("/"))
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
 * Create a wrapper that provides WebSocket context with custom data.
 */
export function createWsWrapper(overrides: Partial<WsData> = {}) {
  const wsData = { ...defaultWsData, ...overrides };

  // We provide the context via a mock of the useWs hook
  return function WsWrapper({ children }: { children: React.ReactNode }) {
    return <>{children}</>;
  };
}
