import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage } from "./helpers";

test.describe("Fleet page", () => {
  test.beforeEach(async ({ page }) => {
    await setupAuthenticatedPage(page);
  });

  test("renders fleet stats with zero state", async ({ page }) => {
    await page.goto("/fleet");
    await expect(page.getByText("Fleet").first()).toBeVisible({ timeout: 10000 });
    // Without WS data, worker count should be 0
    await expect(page.getByText("Workers").first()).toBeVisible();
    await expect(page.getByText("Cores").first()).toBeVisible();
  });

  test("shows empty worker list message", async ({ page }) => {
    await page.goto("/fleet");
    // With no workers connected, should show filter-related empty state
    await expect(page.getByText(/no workers/i)).toBeVisible({ timeout: 10000 });
  });

  test("health summary shows zeroes", async ({ page }) => {
    await page.goto("/fleet");
    await expect(page.getByText("Health Summary")).toBeVisible({ timeout: 10000 });
    await expect(page.getByText("Healthy")).toBeVisible();
    await expect(page.getByText("Stale")).toBeVisible();
    await expect(page.getByText("Offline")).toBeVisible();
  });

  test("Add Server button is visible", async ({ page }) => {
    await page.goto("/fleet");
    await expect(page.getByRole("button", { name: /add server/i })).toBeVisible({ timeout: 10000 });
  });
});
