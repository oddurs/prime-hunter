import { test, expect } from "@playwright/test";
import { mockAuth, mockAuthApi, mockSupabaseData } from "./helpers";

const SUPABASE_URL = "https://nljvgyorzoxajodkkqdu.supabase.co";

test.describe("Login page", () => {
  test("renders email and password form when unauthenticated", async ({ page }) => {
    // Ensure no auth session — intercept getSession to return empty
    await page.route(`${SUPABASE_URL}/auth/v1/**`, (route) =>
      route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { session: null } }),
      }),
    );
    await page.goto("/");
    // Login form uses placeholder text, not labels with htmlFor
    await expect(page.getByPlaceholder("you@example.com")).toBeVisible({ timeout: 10000 });
    await expect(page.getByPlaceholder("Password")).toBeVisible();
    await expect(page.getByRole("button", { name: /sign in/i })).toBeVisible();
  });

  test("shows error on invalid credentials", async ({ page }) => {
    // Auth endpoint returns no session initially
    await page.route(`${SUPABASE_URL}/auth/v1/**`, (route) => {
      const url = route.request().url();
      if (url.includes("/token")) {
        return route.fulfill({
          status: 400,
          contentType: "application/json",
          body: JSON.stringify({
            error: "invalid_grant",
            error_description: "Invalid login credentials",
          }),
        });
      }
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({ data: { session: null } }),
      });
    });

    await page.goto("/");
    await page.getByPlaceholder("you@example.com").fill("wrong@example.com");
    await page.getByPlaceholder("Password").fill("badpassword");
    await page.getByRole("button", { name: /sign in/i }).click();

    // Should show an error message
    await expect(page.getByText(/invalid|error|failed/i)).toBeVisible({ timeout: 10000 });
  });

  test("redirects to dashboard after successful login", async ({ page }) => {
    await mockAuth(page);
    await mockAuthApi(page);
    await mockSupabaseData(page);

    await page.goto("/");
    // With mocked auth session in localStorage, should see the dashboard
    await expect(page.getByText("Dashboard").first()).toBeVisible({ timeout: 10000 });
  });
});
