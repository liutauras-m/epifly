import { test, expect } from '@playwright/test';

test.describe('shell: tabs', () => {
  test.skip(!process.env.TAURI_WEBDRIVER_URL, 'requires TAURI_WEBDRIVER_URL');

  test.beforeEach(async ({ page }) => {
    await page.goto('tauri://localhost');
    // Wait for shell to be ready
    await expect(page.locator('.shell-content')).toBeVisible({ timeout: 15_000 });
  });

  test('TabStrip renders with create button', async ({ page }) => {
    await expect(page.getByRole('button', { name: /new tab/i })).toBeVisible();
  });

  test('create tab adds entry to TabStrip', async ({ page }) => {
    const before = await page.locator('[role="tab"]').count();
    await page.getByRole('button', { name: /new tab/i }).click();
    await expect(page.locator('[role="tab"]')).toHaveCount(before + 1, { timeout: 5000 });
  });

  test('close tab removes entry from TabStrip', async ({ page }) => {
    await page.getByRole('button', { name: /new tab/i }).click();
    const tabs = page.locator('[role="tab"]');
    const count = await tabs.count();
    // Click close on the last tab
    await tabs.last().getByRole('button', { name: /close/i }).click();
    await expect(tabs).toHaveCount(count - 1, { timeout: 5000 });
  });

  test('recorder controls are visible in sidebar', async ({ page }) => {
    await expect(page.getByRole('button', { name: /start|record/i })).toBeVisible();
  });

  test('status dot shows connected after shell-ready', async ({ page }) => {
    // Shell emits shell-ready after startup; dot should be green (ready class)
    await expect(page.locator('.status-dot.ready')).toBeVisible({ timeout: 10_000 });
  });
});
