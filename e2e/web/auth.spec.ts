import { test, expect } from '@playwright/test';
import { signIn } from '../helpers/auth';

// Sign-in runs the real BFF OIDC flow against e2e/fixtures/mock-oidc.mjs.
test.describe('auth', () => {
  test('redirects an unauthenticated user to the IdP via /auth/login', async ({ page }) => {
    const res = await page.request.get('/', { maxRedirects: 0 });
    expect(res.status()).toBe(303);
    expect(res.headers()['location']).toBe('/auth/login?returnTo=%2F');
  });

  test('sign-in reaches the workshop and sets only the encrypted host-only cookie', async ({ page }) => {
    await signIn(page);
    const cookies = await page.context().cookies();
    const session = cookies.find((c) => c.name === '__Host-epifly_session');
    expect(session).toBeDefined();
    expect(session).toMatchObject({ httpOnly: true, secure: true, sameSite: 'Lax', path: '/' });
    expect(cookies.some((c) => c.name.startsWith('conusai_'))).toBe(false);
    expect(await page.evaluate(() => document.cookie)).toBe('');
  });

  test('returns to the requested page after sign-in', async ({ page }) => {
    await signIn(page, '/account');
    await expect(page.getByRole('heading', { name: 'Account' })).toBeVisible();
  });

  test('a forged (unsigned JWT) session cookie is treated as signed out', async ({ request }) => {
    const res = await request.get('/', {
      maxRedirects: 0,
      headers: { cookie: '__Host-epifly_session=eyJhbGciOiJub25lIn0.eyJzdWIiOiJ4IiwiZXhwIjo5OTk5OTk5OTk5fQ.' }, // gitleaks:allow (forged test value)
    });
    expect(res.status()).toBe(303);
    expect(res.headers()['set-cookie']).toContain('__Host-epifly_session=;');
  });

  test('the gateway proxy refuses a cross-origin POST', async ({ page }) => {
    await signIn(page);
    const res = await page.request.post('/v1/threads', { headers: { origin: 'https://evil.example' }, data: {} });
    expect(res.status()).toBe(403);
  });

  test('sign-out ends the session at the IdP and lands on the signed-out page', async ({ page }) => {
    await signIn(page, '/account');
    await page.getByRole('button', { name: /Sign out/ }).click();
    await expect(page).toHaveURL(/\/auth\/signed-out$/);
    const res = await page.request.get('/', { maxRedirects: 0 });
    expect(res.status()).toBe(303);
  });
});
