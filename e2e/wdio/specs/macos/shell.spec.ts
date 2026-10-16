// SKIPPED (2026-10-16): this suite signed in by forging the removed HMAC
// `conusai_session` cookie / `x-session-token`. The native shell now signs in
// with Zitadel through the system browser (loopback PKCE, refresh token in the
// Rust Stronghold vault), which Appium cannot drive. Re-enable once the suite
// has a test Zitadel user and a sign-in step for the system browser.
/**
 * macOS browser-shell smoke tests — driven by tauri-webdriver.
 *
 * The shell loads the SvelteKit web app inside a WKWebView. These tests
 * verify the app renders the login screen and can complete the entry flow,
 * confirming hydration + DOM event listeners work in the real Tauri runtime
 * (where Playwright + WebKit emulation cannot reach).
 */

import { browser, $, expect } from '@wdio/globals';
import { login } from '../../fixtures/login';

describe.skip('browser-shell on macOS', () => {
  it('loads the workshop entry form', async () => {
    // tauri-webdriver loads the app's primary URL automatically when the
    // session starts; navigate explicitly to /login to be deterministic.
    await browser.url('/login');
    const heading = await $('h1, h2');
    await heading.waitForDisplayed({ timeout: 10_000 });
    await expect(heading).toHaveTextContaining('workshop');
  });

  it('login form accepts input and reaches the workshop', async () => {
    await login('Shell Tester', 'Enterprise');
    const greeting = await $('.greeting-text');
    await greeting.waitForDisplayed({ timeout: 10_000 });
    await expect(greeting).toHaveTextContaining('Shell');
  });

  it('user chip shows ENTERPRISE plan', async () => {
    await login('Shell Tester', 'Enterprise');
    const plan = await $('.user-plan');
    await expect(plan).toHaveTextContaining('ENTERPRISE');
  });

  it('theme switcher toggles data-theme attribute', async () => {
    await login('Shell Tester');
    const switcher = await $('button[aria-label*="theme" i], button[aria-label*="Theme" i]');
    await switcher.click();
    const theme = await browser.execute(() => document.documentElement.getAttribute('data-theme'));
    expect(['paper', 'forge']).toContain(theme as string);
  });
});
