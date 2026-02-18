/**
 * Mock Supabase client for testing.
 *
 * Usage in tests:
 *   vi.mock("@/lib/supabase", () => mockSupabaseModule());
 *
 * Then control responses:
 *   mockSupabaseQuery({ data: [...], count: 5, error: null });
 *   mockSupabaseRpc({ data: {...}, error: null });
 */
import { vi } from "vitest";

// Shared response state — tests can set these before calling hooks
let queryResponse: { data: unknown; count: number | null; error: unknown } = {
  data: [],
  count: 0,
  error: null,
};
let rpcResponse: { data: unknown; error: unknown } = {
  data: null,
  error: null,
};
let singleResponse: { data: unknown; error: unknown } = {
  data: null,
  error: null,
};

export function mockSupabaseQuery(resp: typeof queryResponse) {
  queryResponse = resp;
}

export function mockSupabaseRpc(resp: typeof rpcResponse) {
  rpcResponse = resp;
}

export function mockSupabaseSingle(resp: typeof singleResponse) {
  singleResponse = resp;
}

export function resetMockSupabase() {
  queryResponse = { data: [], count: 0, error: null };
  rpcResponse = { data: null, error: null };
  singleResponse = { data: null, error: null };
}

// Chainable query builder
function createQueryBuilder() {
  const builder: Record<string, unknown> = {};
  const chain = (obj: Record<string, unknown>) => {
    obj.select = vi.fn().mockReturnValue(obj);
    obj.eq = vi.fn().mockReturnValue(obj);
    obj.ilike = vi.fn().mockReturnValue(obj);
    obj.gte = vi.fn().mockReturnValue(obj);
    obj.lte = vi.fn().mockReturnValue(obj);
    obj.in = vi.fn().mockReturnValue(obj);
    obj.order = vi.fn().mockReturnValue(obj);
    obj.range = vi.fn().mockReturnValue(obj);
    obj.limit = vi.fn().mockReturnValue(obj);
    obj.insert = vi.fn().mockReturnValue(obj);
    obj.update = vi.fn().mockReturnValue(obj);
    obj.single = vi.fn().mockResolvedValue(singleResponse);
    obj.then = vi.fn((resolve: (v: unknown) => void) =>
      resolve(queryResponse)
    );
    return obj;
  };
  return chain(builder);
}

// Channel mock for realtime
function createChannelMock() {
  const channelMock: Record<string, unknown> = {};
  channelMock.on = vi.fn().mockReturnValue(channelMock);
  channelMock.subscribe = vi.fn().mockReturnValue(channelMock);
  return channelMock;
}

// The mock supabase client
export const mockSupabase = {
  from: vi.fn(() => createQueryBuilder()),
  rpc: vi.fn(() => Promise.resolve(rpcResponse)),
  channel: vi.fn(() => createChannelMock()),
  removeChannel: vi.fn(),
  auth: {
    getSession: vi.fn().mockResolvedValue({
      data: { session: null },
    }),
    onAuthStateChange: vi.fn().mockReturnValue({
      data: { subscription: { unsubscribe: vi.fn() } },
    }),
    signInWithPassword: vi.fn().mockResolvedValue({ error: null }),
    signOut: vi.fn().mockResolvedValue({}),
  },
};

/** Call this to get the vi.mock factory for "@/lib/supabase" */
export function mockSupabaseModule() {
  return { supabase: mockSupabase };
}
