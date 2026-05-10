import { test, expect } from '@playwright/test';

// macOS browser-shell E2E — runs against Tauri app via tauri-driver WebDriver bridge.
// Requires: TAURI_WEBDRIVER_URL env set by the test runner (see e2e/helpers/tauri.ts).

test.describe('shell: login', () => {
  test('LoginPanel renders when no device token is set', async ({ page }) => {
    await page.goto('tauri://localhost');
    await expect(page.getByText('ConusAI Browser Shell')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByLabel('Device token')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Connect' })).toBeVisible();
  });

  test('Connect button is disabled when token field is empty', async ({ page }) => {
    await page.goto('tauri://localhost');
    await expect(page.getByRole('button', { name: 'Connect' })).toBeDisabled();
  });

  test('invalid token shows error message', async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.getByLabel('Device token').fill('invalid-token');
    await page.getByRole('button', { name: 'Connect' }).click();
    await expect(page.getByRole('alert')).toBeVisible({ timeout: 5000 });
  });

  test('E2E bypass: CONUSAI_E2E=1 pre-authenticates', async ({ page }) => {
    // When launched with CONUSAI_E2E=1 env, the shell skips LoginPanel
    // and shows the workspace view directly. This is gated via device_auth bypass.
    if (!process.env.CONUSAI_E2E) test.skip();
    await page.goto('tauri://localhost');
    await expect(page.locator('.shell-workspace')).toBeVisible({ timeout: 10_000 });
  });
});
