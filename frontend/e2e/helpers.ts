import { type Page } from "@playwright/test";

const SUPABASE_URL = "https://nljvgyorzoxajodkkqdu.supabase.co";
const STORAGE_KEY = "sb-nljvgyorzoxajodkkqdu-auth-token";

/** Fake auth session injected into localStorage so the AuthProvider sees a user. */
const MOCK_SESSION = {
  access_token: "fake-access-token",
  token_type: "bearer",
  expires_in: 86400,
  expires_at: Math.floor(Date.now() / 1000) + 86400,
  refresh_token: "fake-refresh-token",
  user: {
    id: "00000000-0000-0000-0000-000000000001",
    aud: "authenticated",
    role: "authenticated",
    email: "test@primehunt.dev",
    email_confirmed_at: "2025-01-01T00:00:00Z",
    created_at: "2025-01-01T00:00:00Z",
    updated_at: "2025-01-01T00:00:00Z",
    app_metadata: { provider: "email" },
    user_metadata: {},
  },
};

/** Inject a fake auth session into localStorage before the page loads. */
export async function mockAuth(page: Page) {
  await page.addInitScript(
    ({ key, session }) => {
      localStorage.setItem(key, JSON.stringify(session));
    },
    { key: STORAGE_KEY, session: MOCK_SESSION },
  );
}

/** Mock Supabase auth API endpoints so getSession / onAuthStateChange work. */
export async function mockAuthApi(page: Page) {
  // getSession reads from storage first, but the SDK also calls the API for refresh
  await page.route(`${SUPABASE_URL}/auth/v1/**`, (route) => {
    const url = route.request().url();
    if (url.includes("/token")) {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(MOCK_SESSION),
      });
    }
    if (url.includes("/user")) {
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(MOCK_SESSION.user),
      });
    }
    // Default: return empty OK
    return route.fulfill({ status: 200, body: "{}" });
  });
}

export const MOCK_PRIMES = [
  { id: 1, form: "Factorial", expression: "27!+1", digits: 29, found_at: "2025-06-01T12:00:00Z", proof_method: "Pocklington", verified: true },
  { id: 2, form: "Factorial", expression: "37!-1", digits: 44, found_at: "2025-06-02T14:00:00Z", proof_method: "Morrison", verified: true },
  { id: 3, form: "KBN", expression: "3*2^127-1", digits: 39, found_at: "2025-06-03T10:00:00Z", proof_method: "LLR", verified: false },
];

export const MOCK_STATS = {
  total: 42,
  largest_expression: "3*2^127-1",
  largest_digits: 39,
  by_form: [
    { form: "Factorial", count: 20 },
    { form: "KBN", count: 15 },
    { form: "Palindromic", count: 7 },
  ],
};

export const MOCK_TIMELINE = [
  { bucket: "2025-06-01", form: "Factorial", count: 5 },
  { bucket: "2025-06-02", form: "KBN", count: 3 },
];

export const MOCK_DISTRIBUTION = [
  { bucket: 10, form: "Factorial", count: 8 },
  { bucket: 20, form: "KBN", count: 6 },
  { bucket: 30, form: "Palindromic", count: 4 },
];

/** Mock all Supabase REST/RPC endpoints with test data. */
export async function mockSupabaseData(page: Page) {
  await page.route(`${SUPABASE_URL}/rest/v1/rpc/get_stats`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_STATS),
    }),
  );

  await page.route(`${SUPABASE_URL}/rest/v1/rpc/get_discovery_timeline`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_TIMELINE),
    }),
  );

  await page.route(`${SUPABASE_URL}/rest/v1/rpc/get_digit_distribution`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_DISTRIBUTION),
    }),
  );

  // Primes query — mock the Supabase PostgREST response
  await page.route(`${SUPABASE_URL}/rest/v1/primes**`, (route) => {
    const url = route.request().url();
    // Detail query (select=* with id filter)
    if (url.includes("select=*") && url.includes("id=eq.")) {
      const detail = {
        ...MOCK_PRIMES[0],
        search_params: JSON.stringify({ start: 1, end: 100 }),
        verification_tier: 1,
        verification_method: "GMP MR-25",
        verified_at: "2025-06-01T12:30:00Z",
      };
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        headers: { "content-range": "0-0/1" },
        body: JSON.stringify([detail]),
      });
    }
    // List query
    return route.fulfill({
      status: 200,
      contentType: "application/json",
      headers: { "content-range": `0-${MOCK_PRIMES.length - 1}/${MOCK_PRIMES.length}` },
      body: JSON.stringify(MOCK_PRIMES),
    });
  });
}

/** Set up all mocks needed for an authenticated page with data. */
export async function setupAuthenticatedPage(page: Page) {
  await mockAuth(page);
  await mockAuthApi(page);
  await mockSupabaseData(page);
}
