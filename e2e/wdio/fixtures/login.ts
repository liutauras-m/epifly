/**
 * Shared sign-in helper for WDIO suites that drive the web app (iOS Safari).
 * Runs the real BFF OIDC flow against the mock IdP (e2e/fixtures/mock-oidc.mjs),
 * which approves at once — the same flow as e2e/helpers/auth.ts.
 */

import { browser } from '@wdio/globals';

export async function login(returnTo = '/') {
  await browser.url(`/auth/login?returnTo=${encodeURIComponent(returnTo)}`);

  // Wait for hydration marker that the layout sets via $effect
  await browser.waitUntil(
    async () => (await browser.execute(() => document.documentElement.dataset.hydrated)) === 'true',
    { timeout: 15_000, timeoutMsg: 'Layout never hydrated (no [data-hydrated] on :root)' },
  );
}
