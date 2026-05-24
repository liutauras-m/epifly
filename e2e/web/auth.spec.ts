import { test, expect } from '@playwright/test';

test.describe('auth', () => {
  test('redirects unauthenticated user to /login', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/);
  });

  test('login page renders workshop entry form', async ({ page }) => {
    await page.goto('/login');
    await expect(page.getByLabel('Operator name')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Begin' })).toBeVisible();
  });

  test('login with valid credentials reaches workshop', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Operator name').fill('E2E Operator');
    await page.getByLabel('Enterprise').check();
    await page.getByRole('button', { name: 'Begin' }).click();
    await expect(page).toHaveURL('/');
    await expect(page.getByText(/Good/)).toBeVisible();
  });

  test('logout returns to login', async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Operator name').fill('E2E Operator');
    await page.getByRole('button', { name: 'Begin' }).click();
    await expect(page).toHaveURL('/');
    await page.getByRole('link', { name: 'Logout' }).first().click();
    await expect(page).toHaveURL(/\/login/);
  });
});
