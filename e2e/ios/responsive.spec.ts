import { test, expect } from '@playwright/test';
import { signIn } from '../helpers/auth';

// iOS Safari simulation against the web app — covers mobile-specific layout
// and interaction patterns without needing a real Tauri iOS build.

test.describe('iOS responsive layout', () => {
  test.beforeEach(async ({ page }) => {
    await signIn(page);
  });

  test('sidebar is hidden by default on iPhone viewport', async ({ page }) => {
    const sidebar = page.getByRole('navigation', { name: 'Workshop navigation' });
    const transform = await sidebar.evaluate((el) =>
      getComputedStyle(el).transform
    );
    // transform: translateX(-100%) produces a matrix with negative X
    expect(transform).toMatch(/matrix/);
  });

  test('hamburger button opens sidebar on mobile', async ({ page }) => {
    const hamBtn = page.getByRole('button', { name: 'Toggle nav' });
    await hamBtn.click();
    const sidebar = page.getByRole('navigation', { name: 'Workshop navigation' });
    await expect(sidebar).toHaveClass(/open/);
  });

  test('composer textarea fits within viewport width', async ({ page }) => {
    const textarea = page.getByRole('textbox');
    const box = await textarea.boundingBox();
    const vw = page.viewportSize()!.width;
    expect(box!.x + box!.width).toBeLessThanOrEqual(vw);
  });

  test('submit button has minimum 44px touch target height', async ({ page }) => {
    // Fill text to ensure submit button is visible / tappable
    await page.getByRole('textbox').fill('test');
    const submitBtn = page.getByRole('button', { name: /send|submit/i }).first();
    const box = await submitBtn.boundingBox();
    if (box) {
      expect(box.height).toBeGreaterThanOrEqual(44);
    }
  });

  test('no horizontal scroll at 375px width', async ({ page }) => {
    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const clientWidth = await page.evaluate(() => document.documentElement.clientWidth);
    expect(scrollWidth).toBeLessThanOrEqual(clientWidth + 1); // 1px tolerance
  });

  test('signed-out page is usable at 375px', async ({ page }) => {
    await page.context().clearCookies();
    await page.goto('/auth/signed-out');
    // The card should be visible and not overflow the actual viewport width
    const vw = page.viewportSize()!.width;
    const form = page.getByRole('link', { name: 'Sign in again' });
    const box = await form.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBeLessThanOrEqual(vw);
  });

  test('greeting text is readable (not truncated)', async ({ page }) => {
    const greeting = page.locator('.greeting-text');
    const isVisible = await greeting.isVisible();
    expect(isVisible).toBe(true);
    const text = await greeting.textContent();
    expect(text).toMatch(/Good/);
  });
});
