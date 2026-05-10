/**
 * Shared login helper for WDIO suites (macOS shell + iOS Safari).
 * Mirrors the flow used by Playwright tests in e2e/web and e2e/ios.
 */

import { browser, $ } from '@wdio/globals';

export async function login(name = 'WDIO Tester', plan: 'Free' | 'Pro' | 'Enterprise' = 'Enterprise') {
  await browser.url('/login');

  // Fill operator name
  const nameInput = await $('input[name="name"]');
  await nameInput.waitForDisplayed({ timeout: 10_000 });
  await nameInput.setValue(name);

  // Select plan tier (radio with label text matching tier)
  const planRadio = await $(`input[type="radio"][value="${plan.toLowerCase()}"]`);
  await planRadio.click();

  // Submit
  const beginBtn = await $('button[type="submit"]');
  await beginBtn.click();

  // Wait for hydration marker that the layout sets via $effect
  await browser.waitUntil(
    async () => (await browser.execute(() => document.documentElement.dataset.hydrated)) === 'true',
    { timeout: 15_000, timeoutMsg: 'Layout never hydrated (no [data-hydrated] on :root)' },
  );
}
