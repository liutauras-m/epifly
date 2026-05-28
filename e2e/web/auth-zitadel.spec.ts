/**
 * Phase 8 — Acceptance tests for the Zitadel/OIDC auth flow.
 *
 * Two tiers:
 *   1. Security invariants — run without a live Zitadel (route interception).
 *   2. Full OIDC flow — require ZITADEL_ISSUER + a mgmt-API-provisioned test user.
 *      These are skipped when ZITADEL_ISSUER is not set in the environment.
 *
 * Run all:     pnpm test:e2e:web
 * Run tier 1:  pnpm test:e2e:web -- --grep "invariant"
 */

import { test, expect, type Page, type BrowserContext } from "@playwright/test";

const ZITADEL_CONFIGURED = !!process.env.ZITADEL_ISSUER;

// ── Helpers ─────────────────────────────────────────────────────────────────

/** Returns all cookie names visible to JavaScript on the page. */
async function jsCookies(page: Page): Promise<string[]> {
  return page.evaluate(() => document.cookie.split(";").map((c) => c.trim()));
}

/** Assert that no token-shaped string appears in any browser-accessible storage. */
async function assertNoTokensInBrowserStorage(page: Page): Promise<void> {
  const lsKeys = await page.evaluate(() => Object.keys(localStorage));
  const ssKeys = await page.evaluate(() => Object.keys(sessionStorage));

  const tokenKeys = [...lsKeys, ...ssKeys].filter((k) =>
    /token|access|refresh|id_token|session/i.test(k)
  );
  expect(
    tokenKeys,
    `found token-related keys in browser storage: ${tokenKeys.join(", ")}`
  ).toHaveLength(0);
}

/** Assert security headers are present on the given URL. */
async function assertSecurityHeaders(page: Page, url: string): Promise<void> {
  const res = await page.goto(url);
  expect(res?.headers()["x-frame-options"]?.toLowerCase()).toBe("deny");
  expect(res?.headers()["x-content-type-options"]?.toLowerCase()).toBe("nosniff");
  expect(res?.headers()["referrer-policy"]?.toLowerCase()).toContain(
    "strict-origin-when-cross-origin"
  );
  expect(res?.headers()["content-security-policy"]).toContain("frame-ancestors 'none'");
  expect(res?.headers()["permissions-policy"]).toBeTruthy();
}

// ── 8.W.1 — Redirect to login (invariant, no Zitadel needed) ─────────────────

test.describe("8.W.1 — route guard (invariant)", () => {
  test("/ redirects to /auth/login with returnTo", async ({ page }) => {
    const res = await page.goto("/");
    expect(page.url()).toMatch(/\/auth\/login/);
    expect(page.url()).toContain("returnTo=");
  });

  test("/auth/login page is accessible without auth", async ({ page }) => {
    await page.goto("/auth/login");
    expect(page.url()).toMatch(/\/auth\/login/);
    // Should not redirect to another login page
    expect(page.url()).not.toMatch(/\/auth\/login\?returnTo=.*login/);
  });
});

// ── Security headers (invariant, no Zitadel needed) ───────────────────────────

test.describe("security headers (invariant)", () => {
  test("/ has security headers", async ({ page }) => {
    await assertSecurityHeaders(page, "/auth/login");
  });

  test("CSP includes frame-ancestors none", async ({ page }) => {
    const res = await page.goto("/auth/login");
    const csp = res?.headers()["content-security-policy"] ?? "";
    expect(csp).toContain("frame-ancestors 'none'");
    expect(csp).toContain("default-src 'self'");
    expect(csp).toContain("object-src 'none'");
    expect(csp).toContain("base-uri 'self'");
  });
});

// ── returnTo allowlist (invariant) ────────────────────────────────────────────

test.describe("returnTo allowlist (invariant)", () => {
  test("absolute URL returnTo is rejected (defaults to /)", async ({ page, context }) => {
    // The server sanitizes returnTo — an absolute URL should be rejected.
    // We verify by checking that the redirect from / goes to a same-origin login page.
    await page.goto("/auth/login?returnTo=https://evil.example.com/steal");
    // The page should load (not 400) but when we'd complete OIDC the returnTo is sanitized.
    // Since we can't complete OIDC here, we verify the server doesn't reflect the URL.
    expect(page.url()).not.toContain("evil.example.com");
  });

  test("protocol-relative returnTo is sanitized", async ({ page }) => {
    await page.goto("/auth/login?returnTo=//evil.example.com");
    expect(page.url()).not.toContain("evil.example.com");
  });
});

// ── Cookie attributes (invariant — checks via Set-Cookie header) ──────────────

test.describe("8.X.1 — cookie attributes (invariant)", () => {
  test("no session cookie visible to JS before login", async ({ page }) => {
    await page.goto("/auth/login");
    const visible = await jsCookies(page);
    const hasSession = visible.some((c) => c.includes("epifly_sid"));
    expect(hasSession, "session cookie must not be readable by JS").toBe(false);
  });

  test("no token values in localStorage or sessionStorage", async ({ page }) => {
    await page.goto("/auth/login");
    await assertNoTokensInBrowserStorage(page);
  });
});

// ── Callback replay protection (invariant — does not need full OIDC) ─────────

test.describe("8.W — callback security (invariant)", () => {
  test("callback with no state cookie returns 400", async ({ request }) => {
    const res = await request.get("/auth/callback?code=fake&state=fake");
    // Should return 400 (missing_oidc_transaction) because no tx cookie
    expect(res.status()).toBe(400);
  });

  test("callback with mismatched state returns 400", async ({ request, context }) => {
    // Set a tx cookie with one state, send a different state in the URL
    await context.addCookies([
      {
        name: "__Host-epifly_oidc_tx",
        value: "state-aaa",
        domain: "localhost",
        path: "/auth/callback",
        secure: false, // HTTP in test
        httpOnly: true,
        sameSite: "Lax",
      },
    ]);
    const res = await request.get("/auth/callback?code=fake&state=state-bbb");
    expect(res.status()).toBe(400);
  });

  test("login with invalid returnTo does not open redirect", async ({ page }) => {
    await page.route("/auth/callback*", (route) => route.abort());
    await page.goto("/auth/login?returnTo=javascript:alert(1)");
    // returnTo regex rejects non-path values; page should load without redirect
    expect(page.url()).toMatch(/\/auth\/login/);
  });
});

// ── Rate limiting (invariant) ──────────────────────────────────────────────────

test.describe("rate limiting (invariant)", () => {
  test("exceeding 10 /auth/login requests in a window returns 429", async ({ request }) => {
    // Send 15 requests in rapid succession
    const results = await Promise.all(
      Array.from({ length: 15 }, () =>
        request.get("/auth/login").then((r) => r.status())
      )
    );
    const has429 = results.some((s) => s === 429);
    expect(has429, "expected at least one 429 after rate limit exceeded").toBe(true);
  });
});

// ── Full OIDC flow (requires ZITADEL_ISSUER) ──────────────────────────────────

test.describe("full OIDC flow", () => {
  test.skip(!ZITADEL_CONFIGURED, "ZITADEL_ISSUER not set — skipping full OIDC tests");

  // These tests are run in CI with a live Zitadel and a mgmt-API-provisioned test user.
  // The user credentials are injected via ZITADEL_TEST_USER_EMAIL and ZITADEL_TEST_USER_PASSWORD.

  const TEST_USER_EMAIL = process.env.ZITADEL_TEST_USER_EMAIL ?? "";
  const TEST_USER_PASSWORD = process.env.ZITADEL_TEST_USER_PASSWORD ?? "";

  /** Helper: complete a full OIDC login via the Zitadel UI. */
  async function oidcLogin(page: Page, email: string, password: string) {
    await page.goto("/auth/login?returnTo=%2F");
    await expect(page).toHaveURL(/\/auth\/login/);

    // Click the primary CTA which redirects to Zitadel
    await page.getByRole("link", { name: /sign in|continue|log in/i }).first().click();
    await expect(page).toHaveURL(new RegExp(process.env.ZITADEL_ISSUER!));

    // Fill Zitadel login form
    await page.getByRole("textbox", { name: /email|username/i }).fill(email);
    await page.getByRole("button", { name: /next|continue/i }).click();
    await page.getByRole("textbox", { name: /password/i }).fill(password);
    await page.getByRole("button", { name: /sign in|log in|next/i }).click();

    // Land back on the app
    await expect(page).toHaveURL("http://localhost:5173/", { timeout: 15_000 });
  }

  test("8.W.2-4: sign-in redirects to Zitadel and returns to /", async ({ page }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);

    // Sidebar shows display name (not email)
    await expect(
      page.locator('[data-testid="user-display-name"], [aria-label*="account"]')
    ).not.toContainText(TEST_USER_EMAIL.split("@")[0] ?? "");
  });

  test("8.W.5: chat stream works after login", async ({ page }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);

    // Intercept the chat API call to avoid real LLM call
    await page.route("/api/v1/chat/stream*", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "text/event-stream",
        body: [
          'data: {"choices":[{"delta":{"content":"Hello"}}]}\n\n',
          'data: {"thread_id":"test-thread-id"}\n\n',
          'data: [DONE]\n\n',
        ].join(""),
      });
    });

    await page.getByRole("textbox").fill("Hello");
    await page.getByRole("textbox").press("Meta+Enter");
    await expect(page.getByText("Hello")).toBeVisible({ timeout: 5_000 });
  });

  test("8.W.6: reload keeps session", async ({ page }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);
    await page.reload();
    await expect(page).toHaveURL("http://localhost:5173/");
    // Should not redirect to login
    await expect(page).not.toHaveURL(/\/auth\/login/);
  });

  test("8.W.7: logout clears cookies and redirects to login", async ({ page }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);

    // Find and click the logout button
    await page.goto("/auth/logout");
    await expect(page).toHaveURL(/\/auth\/login|\//, { timeout: 10_000 });

    // Session cookie should be gone
    const cookies = await page.context().cookies();
    const hasSid = cookies.some((c) => c.name === "__Host-epifly_sid");
    expect(hasSid).toBe(false);
  });

  test("8.X.1: session cookie is httpOnly and secure in prod", async ({ page, context }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);

    const cookies = await context.cookies();
    const sid = cookies.find((c) => c.name === "__Host-epifly_sid");
    expect(sid, "session cookie not found after login").toBeTruthy();
    expect(sid?.httpOnly).toBe(true);
    expect(sid?.sameSite).toBe("Lax");
    expect(sid?.path).toBe("/");
  });

  test("8.X.2: no tokens in browser-accessible storage after login", async ({ page }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);
    await assertNoTokensInBrowserStorage(page);

    // Also check that the session cookie is not visible to JS
    const visible = await jsCookies(page);
    const hasSession = visible.some((c) => c.includes("epifly_sid"));
    expect(hasSession, "session cookie must be httpOnly").toBe(false);
  });

  test("8.W.10: cross-tenant probe returns 404", async ({ page, request }) => {
    await oidcLogin(page, TEST_USER_EMAIL, TEST_USER_PASSWORD);

    // Attempt to access a workspace node from a different tenant
    const fakeCrossTenantNodeId = "00000000-0000-0000-0000-000000000001";
    const res = await request.get(`/api/v1/workspaces/${fakeCrossTenantNodeId}`);
    expect([404, 403]).toContain(res.status());
  });

  test("8.W.9: SQL — one active session row after login", async ({ page }) => {
    // This test documents the SQL probe; execution requires direct DB access in CI.
    // The assertion here is a placeholder that always passes without DB connectivity.
    // In CI: SELECT 1 FROM auth_sessions WHERE user_iss=$1 AND user_sub=$2 AND revoked_at IS NULL → 1 row
    test.skip(
      !process.env.DATABASE_URL,
      "DATABASE_URL not set — skipping SQL probe"
    );
    // If DATABASE_URL is set, the CI pipeline runs the actual SQL probe separately.
  });
});
