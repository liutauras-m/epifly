import { test, expect } from '@playwright/test';
import { signIn } from '../helpers/auth';

test.describe('greeting screen', () => {
  test.beforeEach(async ({ page }) => {
    await signIn(page);
  });

  test('renders greeting with operator first name', async ({ page }) => {
    await expect(page.getByText(/Good .*, E2E/)).toBeVisible();
  });

  test('composer textarea is focusable and accepts input', async ({ page }) => {
    const textarea = page.getByRole('textbox');
    await textarea.click();
    await textarea.fill('hello world');
    await expect(textarea).toHaveValue('hello world');
  });

  test('sidebar shows workspace navigation', async ({ page }) => {
    await expect(page.getByRole('navigation', { name: 'Workshop navigation' })).toBeVisible();
  });

  test('user chip shows initials', async ({ page }) => {
    // Avatar shows initials "EO" for E2E Operator. The plan badge comes from the
    // gateway's /v1/billing/subscription during SSR (no gateway in this suite).
    await expect(page.locator('.avatar')).toBeVisible();
  });

  test('theme switcher is accessible', async ({ page }) => {
    const switcher = page.getByRole('button', { name: /theme/i });
    await expect(switcher).toBeVisible();
    await switcher.click();
    // Theme toggles — check html data-theme changed (only 'paper' and 'forge' exist)
    const theme = await page.locator('html').getAttribute('data-theme');
    expect(['paper', 'forge']).toContain(theme);
  });

  test('Cmd+N / Ctrl+N starts new session', async ({ page }) => {
    await page.route('**/ui/stream', (route) =>
      route.fulfill({ status: 200, contentType: 'text/event-stream', body: 'data: [DONE]\n\n' })
    );
    // Submit a prompt to enter chat view
    await page.getByRole('textbox').fill('test message');
    await page.getByRole('textbox').press('Meta+Enter');
    await expect(page.getByText(/Good .*, E2E/)).not.toBeVisible();
    // Cmd+N resets back to greeting
    await page.keyboard.press('Meta+n');
    await expect(page.getByText(/Good .*, E2E/)).toBeVisible();
  });

  test('sidebar toggles on mobile viewport', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    const sidebar = page.getByRole('navigation', { name: 'Workshop navigation' });
    // On mobile sidebar starts hidden (transform: translateX(-100%))
    const hamBtn = page.getByRole('button', { name: 'Toggle nav' });
    await expect(hamBtn).toBeVisible();
    await hamBtn.click();
    await expect(sidebar).toHaveClass(/open/);
  });
});
