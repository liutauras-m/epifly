import { test, expect } from '@playwright/test';

// macOS browser-shell E2E — runs against Tauri app via tauri-driver WebDriver bridge.
// Requires: TAURI_WEBDRIVER_URL env set by the test runner (see e2e/helpers/tauri.ts).
//
// Only the signed-out state is covered: sign-in opens Zitadel in the system
// browser (loopback PKCE), which WebDriver cannot drive. The full flow needs a
// test Zitadel user and is not automated yet.

test.describe('shell: sign-in screen', () => {
  test.beforeEach(async () => {
    if (!process.env.TAURI_WEBDRIVER_URL) test.skip();
  });

  test('shows the sign-in screen when the vault holds no session', async ({ page }) => {
    await page.goto('tauri://localhost');
    await expect(page.getByRole('heading', { name: /workshop/i })).toBeVisible({ timeout: 10_000 });
    await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
  });

  test('keeps no token or user in webview storage', async ({ page }) => {
    await page.goto('tauri://localhost');
    const stored = await page.evaluate(() => ({
      local: Object.keys(localStorage),
      session: Object.keys(sessionStorage),
      cookie: document.cookie,
    }));
    expect(stored.local.filter((k) => /token|session|user/i.test(k))).toEqual([]);
    expect(stored.session).toEqual([]);
    expect(stored.cookie).toBe('');
  });
});
