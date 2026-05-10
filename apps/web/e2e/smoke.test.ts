import { test, expect } from "@playwright/test";

test.describe("smoke", () => {
  test("homepage loads and has correct title", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/ConusAI/i);
  });

  test("login page renders without JS errors", async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (err) => errors.push(err.message));

    await page.goto("/login");

    // Should have a login form
    const form = page.getByRole("form").or(page.locator("form"));
    await expect(form).toBeVisible();

    expect(errors).toHaveLength(0);
  });

  test("unauthenticated root redirects to login", async ({ page }) => {
    const response = await page.goto("/");
    // Either a redirect to /login or the login page rendered at /
    const url = page.url();
    const status = response?.status() ?? 200;
    expect(status).toBeLessThan(400);
    const isLoginPage =
      url.includes("/login") || (await page.locator("form").count()) > 0;
    expect(isLoginPage).toBe(true);
  });

  test("CSP header is present", async ({ page }) => {
    const response = await page.goto("/login");
    const csp = response?.headers()["content-security-policy"];
    expect(csp).toBeTruthy();
    expect(csp).toContain("default-src");
  });
});
