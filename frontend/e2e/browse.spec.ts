import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage, MOCK_PRIMES } from "./helpers";

test.describe("Browse page", () => {
  test.beforeEach(async ({ page }) => {
    await setupAuthenticatedPage(page);
  });

  test("renders filter controls and primes table", async ({ page }) => {
    await page.goto("/browse");
    // Should see search input
    await expect(page.getByPlaceholder(/expression/i)).toBeVisible({ timeout: 10000 });
    // Should see prime data in table
    await expect(page.getByText("27!+1")).toBeVisible();
  });

  test("search input updates URL with query param", async ({ page }) => {
    await page.goto("/browse");
    await page.getByPlaceholder(/expression/i).fill("27!");
    // Wait for debounce
    await page.waitForTimeout(500);
    // URL should contain the search query
    expect(page.url()).toContain("q=27");
  });

  test("pagination controls are present", async ({ page }) => {
    await page.goto("/browse");
    // Should see Previous button (disabled on first page)
    await expect(page.getByRole("button", { name: /previous/i })).toBeVisible({ timeout: 10000 });
    await expect(page.getByRole("button", { name: /previous/i })).toBeDisabled();
    // Next button should also be present (exact match to avoid Next.js dev tools)
    await expect(page.getByRole("button", { name: "Next", exact: true })).toBeVisible();
  });

  test("clicking a row opens detail dialog", async ({ page }) => {
    await page.goto("/browse");
    // Wait for table to render
    await expect(page.getByText("27!+1")).toBeVisible({ timeout: 10000 });
    // Click the first prime row
    await page.getByText("27!+1").click();
    // Detail dialog should open — look for "Loading..." or the detail content
    await expect(page.locator('[role="dialog"]')).toBeVisible({ timeout: 5000 });
  });
});
