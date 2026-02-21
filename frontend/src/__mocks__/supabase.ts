/**
 * @file Mock Supabase client for testing
 * @module __mocks__/supabase
 *
 * Provides a comprehensive mock of the Supabase client used throughout the
 * darkreach frontend. This mock supports all three Supabase interaction patterns:
 *
 * 1. **Query builder** (from().select().eq().order().range()): Returns chainable
 *    mock objects that resolve with configurable response data. Supports all
 *    filter methods (eq, ilike, gte, lte, in), ordering, pagination (range/limit),
 *    mutations (insert, update), and single-row queries.
 *
 * 2. **RPC calls** (rpc("function_name", params)): Returns a Promise resolving
 *    with configurable data/error response.
 *
 * 3. **Realtime channels** (channel().on().subscribe()): Returns chainable mock
 *    objects for Realtime postgres_changes subscriptions.
 *
 * 4. **Auth** (auth.getSession, auth.onAuthStateChange, etc.): Provides mock
 *    implementations for the full Supabase Auth lifecycle.
 *
 * Usage pattern:
 * ```typescript
 * // In test setup:
 * vi.mock("@/lib/supabase", () => mockSupabaseModule());
 *
 * // Configure responses before rendering hooks:
 * mockSupabaseQuery({ data: [...], count: 5, error: null });
 * mockSupabaseRpc({ data: {...}, error: null });
 * ```
 *
 * Note: Most hook tests in this codebase use inline mocks rather than this
 * shared mock file, because inline mocks provide more precise control over
 * the chain resolution behavior. This shared mock is best for simpler tests.
 *
 * @see {@link ../lib/supabase} Real Supabase client singleton
 */
import { vi } from "vitest";

// Shared response state -- tests can set these before calling hooks.
// Each response type corresponds to a different Supabase interaction pattern.
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

/** Configure the response for query builder chains (from().select()...range()). */
export function mockSupabaseQuery(resp: typeof queryResponse) {
  queryResponse = resp;
}

/** Configure the response for RPC calls (supabase.rpc()). */
export function mockSupabaseRpc(resp: typeof rpcResponse) {
  rpcResponse = resp;
}

/** Configure the response for single-row queries (.single()). */
export function mockSupabaseSingle(resp: typeof singleResponse) {
  singleResponse = resp;
}

/** Reset all mock responses to their default empty/null state. */
export function resetMockSupabase() {
  queryResponse = { data: [], count: 0, error: null };
  rpcResponse = { data: null, error: null };
  singleResponse = { data: null, error: null };
}

/**
 * Creates a chainable query builder mock that simulates the Supabase
 * PostgREST query API. Every method returns the same object for chaining,
 * except .single() which resolves with singleResponse and .then() which
 * resolves with queryResponse (for awaited chains).
 */
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

/**
 * Creates a mock Supabase Realtime channel with chainable .on() and .subscribe().
 * Simulates: supabase.channel("name").on("postgres_changes", filter, callback).subscribe()
 */
function createChannelMock() {
  const channelMock: Record<string, unknown> = {};
  channelMock.on = vi.fn().mockReturnValue(channelMock);
  channelMock.subscribe = vi.fn().mockReturnValue(channelMock);
  return channelMock;
}

/**
 * The mock Supabase client object with all methods pre-configured.
 * Provides: from(), rpc(), channel(), removeChannel(), and auth methods.
 */
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

/**
 * Factory function that returns the vi.mock module replacement for "@/lib/supabase".
 * Usage: `vi.mock("@/lib/supabase", () => mockSupabaseModule())`
 */
export function mockSupabaseModule() {
  return { supabase: mockSupabase };
}
