import { test, expect } from "@playwright/test";

test.describe("Theme toggle", () => {
  test("defaults to dark theme", async ({ page }) => {
    await page.goto("/");
    const html = page.locator("html");
    await expect(html).toHaveClass(/dark/);
  });

  test("persists theme across page reload", async ({ page }) => {
    // Set light theme in localStorage
    await page.addInitScript(() => {
      localStorage.setItem("primehunt-theme", "light");
    });
    await page.goto("/");
    const html = page.locator("html");
    await expect(html).toHaveClass(/light/);

    // Reload and check persistence
    await page.reload();
    await expect(html).toHaveClass(/light/);
  });

  test("dark theme class applied with no localStorage", async ({ page }) => {
    await page.addInitScript(() => {
      localStorage.removeItem("primehunt-theme");
    });
    await page.goto("/");
    const html = page.locator("html");
    await expect(html).toHaveClass(/dark/);
  });
});
