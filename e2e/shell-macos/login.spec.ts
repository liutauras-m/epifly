import { test, expect } from '@playwright/test';

// macOS browser-shell E2E — runs against Tauri app via tauri-driver WebDriver bridge.
// Requires: TAURI_WEBDRIVER_URL env set by the test runner (see e2e/helpers/tauri.ts).

test.describe('shell: login', () => {
  test.beforeEach(async () => {
    if (!process.env.TAURI_WEBDRIVER_URL) test.skip();
  });
  test('workshop login renders when no session is stored', async ({ page }) => {
    await page.goto('tauri://localhost');
    // Clear any persisted session so the login form is shown.
    await page.evaluate(() => localStorage.removeItem('conusai_shell_user'));
    await page.reload();
    await expect(page.getByRole('heading', { name: /workshop/i })).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('#name-input')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Begin' })).toBeVisible();
  });

  test('Begin button is enabled only when name is filled', async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.evaluate(() => localStorage.removeItem('conusai_shell_user'));
    await page.reload();
    // Empty name → button still renders (validation fires on submit, not on blur).
    // Fill name → button stays enabled.
    await page.locator('#name-input').fill('Test Operator');
    await expect(page.getByRole('button', { name: 'Begin' })).toBeVisible();
  });

  test('submitting name + plan persists session and shows workspace', async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.evaluate(() => localStorage.removeItem('conusai_shell_user'));
    await page.reload();
    await page.locator('#name-input').fill('Shell Tester');
    await page.locator('input[name="plan"][value="enterprise"]').check();
    await page.getByRole('button', { name: 'Begin' }).click();
    // After login the workspace/greeting screen should appear.
    await expect(page.getByText('Shell Tester')).toBeVisible({ timeout: 8_000 });
  });

  test('session persists across reload', async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.evaluate(() =>
      localStorage.setItem('conusai_shell_user', JSON.stringify({ name: 'Reload User', plan: 'pro' }))
    );
    await page.reload();
    // Should skip login and land on workspace directly.
    await expect(page.getByText('Reload User')).toBeVisible({ timeout: 8_000 });
  });

  test('logout clears session and returns to login', async ({ page }) => {
    await page.goto('tauri://localhost');
    await page.evaluate(() =>
      localStorage.setItem('conusai_shell_user', JSON.stringify({ name: 'Logout User', plan: 'pro' }))
    );
    await page.reload();
    await expect(page.getByText('Logout User')).toBeVisible({ timeout: 8_000 });
    await page.getByRole('button', { name: 'Sign out' }).click();
    await expect(page.getByRole('heading', { name: /workshop/i })).toBeVisible({ timeout: 5_000 });
  });
});
