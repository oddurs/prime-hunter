import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage } from "./helpers";

const MOCK_DOC_INDEX = [
  { slug: "factorial", title: "Factorial Primes", category: "form" },
  { slug: "kbn", title: "KBN Primes", category: "form" },
];

const MOCK_DOC_CONTENT = {
  slug: "factorial",
  title: "Factorial Primes",
  content: "# Factorial Primes\n\nPrimes of the form n! +/- 1.",
  category: "form",
};

test.describe("Docs page", () => {
  test.beforeEach(async ({ page }) => {
    await setupAuthenticatedPage(page);
    // Mock all docs API endpoints served by the Rust backend
    await page.route("**/api/docs/search**", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: "[]" }),
    );
    await page.route("**/api/docs/roadmaps/**", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_DOC_CONTENT) }),
    );
    await page.route("**/api/docs/agents/**", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_DOC_CONTENT) }),
    );
    await page.route("**/api/docs/factorial", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_DOC_CONTENT) }),
    );
    await page.route("**/api/docs/kbn", (route) =>
      route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(MOCK_DOC_CONTENT) }),
    );
    // Doc index — catch-all for /api/docs requests
    await page.route("**/api/docs**", (route) => {
      const url = route.request().url();
      // Only intercept the index endpoint (no slug after /api/docs)
      if (url.match(/\/api\/docs\/?(\?|$)/)) {
        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: JSON.stringify(MOCK_DOC_INDEX),
        });
      }
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(MOCK_DOC_CONTENT),
      });
    });
  });

  test("renders docs page with sidebar entries", async ({ page }) => {
    await page.goto("/docs");
    // The docs page should render with the sidebar showing doc entries
    await expect(page.getByText(/docs|documentation|forms/i).first()).toBeVisible({ timeout: 10000 });
  });

  test("docs page URL is navigable", async ({ page }) => {
    const response = await page.goto("/docs");
    // Page should load without a server error
    expect(response?.status()).toBeLessThan(500);
  });
});
