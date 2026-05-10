/**
 * Native iOS app tests — ConusAI Browser Shell (.app installed on simulator/device)
 *
 * The app launches into the workshop login screen (name + plan tier → Begin).
 * No device token is required. We test that:
 *   - The native shell launches with a WKWebView context
 *   - The webview renders the workshop login form
 *   - Submitting name + plan navigates to the workspace chat view
 */

import { browser, $, $$ , expect } from '@wdio/globals';

async function switchToWebView() {
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
    const ctxs = await browser.getContexts();
    const ctxIds = ctxs.map((c) => (typeof c === 'string' ? c : c.id));
    expect(ctxIds).toContain('NATIVE_APP');
    expect(ctxIds.some((id) => id.includes('WEBVIEW'))).toBe(true);

    const webview = await $('//XCUIElementTypeWebView');
    await webview.waitForExist({ timeout: 10_000 });
  });

  it('webview renders the workshop login form', async () => {
    await switchToWebView();

    // Workshop heading
    const heading = await $('h1');
    await heading.waitForDisplayed({ timeout: 10_000 });
    const text = await heading.getText();
    expect(text.toLowerCase()).toContain('workshop');

    // Name input field
    const nameInput = await $('#name-input');
    await nameInput.waitForDisplayed({ timeout: 5_000 });

    // Plan radio buttons — at least one should be visible
    const planOptions = await $$('input[name="plan"]');
    expect(planOptions.length).toBeGreaterThanOrEqual(3);

    // Begin button
    const beginBtn = await $('button[type="submit"]');
    await beginBtn.waitForDisplayed({ timeout: 5_000 });
    const btnText = await beginBtn.getText();
    expect(btnText).toContain('Begin');
  });

  it('submitting name + plan enters the workspace', async () => {
    await switchToWebView();

    // Clear any persisted session first.
    await browser.execute(() => localStorage.removeItem('conusai_shell_user'));
    await browser.refresh();
    await browser.waitUntil(
      async () => {
        const h = await $('h1');
        return (await h.isDisplayed()) && (await h.getText()).toLowerCase().includes('workshop');
      },
      { timeout: 10_000, timeoutMsg: 'Login form did not reappear after refresh' },
    );

    // Fill in name.
    const nameInput = await $('#name-input');
    await nameInput.setValue('E2E Tester');

    // Select Enterprise plan (already default, but explicitly set).
    await browser.execute(() => {
      const el = document.querySelector<HTMLInputElement>('input[name="plan"][value="enterprise"]');
      if (el) el.click();
    });

    // Click Begin.
    const beginBtn = await $('button[type="submit"]');
    await beginBtn.click();

    // After login, the greeting screen should appear with the user's first name.
    await browser.waitUntil(
      async () => {
        const body = await browser.execute(() => document.body.innerText);
        return typeof body === 'string' && body.toLowerCase().includes('e2e');
      },
      { timeout: 8_000, timeoutMsg: 'Workspace greeting did not appear after login' },
    );
  });
});
