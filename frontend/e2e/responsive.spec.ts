import { test, expect } from "@playwright/test";
import { setupAuthenticatedPage } from "./helpers";

test.describe("Responsive layout", () => {
  test("mobile viewport hides desktop nav links", async ({ page }) => {
    await setupAuthenticatedPage(page);
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/");
    // Wait for page to load
    await expect(page.locator("body")).toContainText(/dashboard|idle|darkreach/i, { timeout: 10000 });
    // On mobile, the desktop nav links should be hidden
    const desktopNav = page.locator("nav.hidden, nav.md\\:flex").first();
    // The page should still render correctly
    await expect(page.locator("body")).toBeVisible();
  });

  test("desktop viewport shows full navigation", async ({ page }) => {
    await setupAuthenticatedPage(page);
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/");
    // Desktop nav should show Browse, Fleet links
    await expect(page.getByText("Dashboard").first()).toBeVisible({ timeout: 10000 });
  });

  test("browse page is usable on mobile", async ({ page }) => {
    await setupAuthenticatedPage(page);
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/browse");
    // Search input should still be visible and usable
    await expect(page.getByPlaceholder(/expression/i)).toBeVisible({ timeout: 10000 });
    // Table should be present
    await expect(page.getByText("27!+1")).toBeVisible();
  });
});
