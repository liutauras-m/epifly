import { test, expect } from "@playwright/test";

test.describe("smoke", () => {
  test("healthz answers without a session", async ({ request }) => {
    const res = await request.get("/healthz");
    expect(res.status()).toBe(200);
  });

  test("unauthenticated root redirects to OIDC sign-in", async ({ request }) => {
    const res = await request.get("/", { maxRedirects: 0 });
    expect(res.status()).toBe(303);
    expect(res.headers()["location"]).toMatch(/^\/auth\/login\?returnTo=/);
  });

  test("signed-out page renders without JS errors and with a nonce CSP", async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (err) => errors.push(err.message));
    const response = await page.goto("/auth/signed-out");
    await expect(page.getByRole("link", { name: "Sign in again" })).toBeVisible();
    expect(errors).toHaveLength(0);
    const csp = response?.headers()["content-security-policy"] ?? "";
    expect(csp).toContain("connect-src 'self'");
    expect(csp).toMatch(/script-src 'self' 'nonce-/);
    expect(response?.headers()["x-frame-options"]).toBe("DENY");
  });

  test("proxied API without a session is 401, not a redirect", async ({ request }) => {
    const res = await request.get("/v1/threads", { maxRedirects: 0 });
    expect(res.status()).toBe(401);
  });
});
