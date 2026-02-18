import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage } from "./helpers";

test.describe("Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await setupAuthenticatedPage(page);
  });

  test("header nav links are visible on desktop", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("Dashboard").first()).toBeVisible({ timeout: 10000 });
    // Nav should have links to key pages
    const nav = page.locator("header, nav").first();
    await expect(nav).toBeVisible();
  });

  test("navigating to /browse shows Browse heading", async ({ page }) => {
    await page.goto("/browse");
    await expect(page.getByRole("heading", { name: /browse/i })).toBeVisible({ timeout: 10000 });
  });

  test("navigating to /fleet shows Fleet heading", async ({ page }) => {
    await page.goto("/fleet");
    await expect(page.getByRole("heading", { name: /fleet/i })).toBeVisible({ timeout: 10000 });
  });

  test("navigating to /searches shows page content", async ({ page }) => {
    await page.goto("/searches");
    // Searches page should render with some content
    await expect(page.getByText(/search/i).first()).toBeVisible({ timeout: 10000 });
  });
});
