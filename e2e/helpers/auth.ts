import { expect, type Page } from '@playwright/test';

/**
 * Sign in through the real BFF OIDC flow against the mock IdP
 * (e2e/fixtures/mock-oidc.mjs, started by playwright.config.ts), which
 * approves at once and redirects back to /auth/callback.
 */
export async function signIn(page: Page, returnTo = '/'): Promise<void> {
	await page.goto(`/auth/login?returnTo=${encodeURIComponent(returnTo)}`);
	await expect(page).toHaveURL(returnTo);
	await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });
}
