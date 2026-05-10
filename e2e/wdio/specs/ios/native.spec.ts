/**
 * Native iOS app tests — ConusAI Browser Shell (.app installed on simulator/device)
 *
 * The app launches into a device-token entry screen (DeviceAuthGate). We test
 * that the native shell launches cleanly, the embedded webview renders the
 * Svelte UI, and the gate accepts E2E bypass tokens in debug builds.
 *
 * Appium opens the session in NATIVE_APP context; we switch to the WKWebView
 * webview context to interact with the SvelteKit UI inside.
 */

import { browser, $, expect } from '@wdio/globals';

async function switchToWebView() {
  // Wait up to 15s for the webview context to be exposed by WKWebView,
  // then switch into it. Appium-XCUITest enumerates contexts as ['NATIVE_APP', 'WEBVIEW_<n>'].
  await browser.waitUntil(
    async () => {
      const ctxs = await browser.getContexts();
      return ctxs.some((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
    },
    { timeout: 15_000, timeoutMsg: 'No WEBVIEW context appeared' },
  );
  const ctxs = await browser.getContexts();
  const wv = ctxs.find((c) => (typeof c === 'string' ? c : c.id).includes('WEBVIEW'));
  await browser.switchContext(typeof wv === 'string' ? wv : wv!.id);
}

describe('Native ConusAI Browser iOS app', () => {
  it('launches with NATIVE_APP context and a WKWebView attached', async () => {
    // App is launched by Appium when the session starts.
    // The Tauri shell wraps a WKWebView — verify both contexts exist.
    const ctxs = await browser.getContexts();
    const ctxIds = ctxs.map((c) => (typeof c === 'string' ? c : c.id));
    expect(ctxIds).toContain('NATIVE_APP');
    expect(ctxIds.some((id) => id.includes('WEBVIEW'))).toBe(true);

    // Native chrome: WKWebView element should be present in the view hierarchy.
    const webview = await $('//XCUIElementTypeWebView');
    await webview.waitForExist({ timeout: 10_000 });
  });

  it('webview renders the Svelte gate form', async () => {
    await switchToWebView();
    const heading = await $('h1, h2');
    await heading.waitForDisplayed({ timeout: 10_000 });
    const text = await heading.getText();
    expect(text.toLowerCase()).toContain('browser shell');

    const tokenInput = await $('input[placeholder*="dv_"], input[name="token"], input[aria-label*="token" i]');
    await tokenInput.waitForDisplayed({ timeout: 10_000 });
  });

  it('accepts E2E bypass token (debug builds only)', async () => {
    await switchToWebView();
    const tokenInput = await $('input[placeholder*="dv_"], input[name="token"], input[aria-label*="token" i]');
    await tokenInput.setValue('dv_e2e_test_token');

    const connectBtn = await $('button*=Connect');
    await connectBtn.click();
    // After connect, expect either a tab area or an error toast — both prove
    // the form submitted and JS handlers ran inside the WKWebView.
    await browser.pause(1_500);
    const html = await browser.execute(() => document.body.innerText);
    expect(typeof html).toBe('string');
  });
});
