import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage } from "./helpers";

test.describe("Dashboard page", () => {
  test.beforeEach(async ({ page }) => {
    await setupAuthenticatedPage(page);
  });

  test("renders stat cards with data", async ({ page }) => {
    await page.goto("/");
    // Should see total primes from mock stats (42) — use exact match to avoid ambiguity
    await expect(page.getByText("42", { exact: true }).first()).toBeVisible({ timeout: 10000 });
    // Should see largest digits (39 digits)
    await expect(page.getByText("39 digits")).toBeVisible();
  });

  test("renders primes table with mock data", async ({ page }) => {
    await page.goto("/");
    // Should see prime expressions from mock data
    await expect(page.getByText("27!+1")).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("37!-1")).toBeVisible();
  });

  test("renders form badges from stats", async ({ page }) => {
    await page.goto("/");
    // Should see form badges from mock stats
    await expect(page.getByText("Factorial").first()).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("KBN").first()).toBeVisible();
  });

  test("shows idle status when no WebSocket data", async ({ page }) => {
    await page.goto("/");
    // Without WS data, should show idle state
    await expect(page.getByText(/idle/i).first()).toBeVisible({ timeout: 10000 });
  });
});
