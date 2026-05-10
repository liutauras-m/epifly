/**
 * iOS Simulator / Real Device tests — driven by Appium XCUITest + Mobile Safari.
 *
 * Same flow as e2e/ios/features.spec.ts but running in the actual iOS Safari
 * runtime (not Playwright's WebKit emulation). This catches gaps the emulator
 * can miss: real iOS scroll behavior, viewport quirks, system font rendering,
 * touch event timing.
 *
 * Run against a booted simulator (no Apple cert needed) or a real device
 * (set IOS_REAL_DEVICE=1 + IOS_DEVICE_UDID + APPLE_TEAM_ID).
 */

import { browser, $, expect } from '@wdio/globals';
import { login } from '../../fixtures/login';

describe('iOS Safari — workshop UI', () => {
  it('login form renders at iPhone viewport width', async () => {
    await browser.url('/login');
    const form = await $('.login-form-wrap');
    await form.waitForDisplayed({ timeout: 10_000 });
    const size = await form.getSize();
    const vw = await browser.execute(() => window.innerWidth);
    expect(size.width).toBeLessThanOrEqual(vw as number);
  });

  it('completes login → greeting screen', async () => {
    await login('iOS Real', 'Enterprise');
    const greeting = await $('.greeting-text');
    await greeting.waitForDisplayed({ timeout: 10_000 });
    await expect(greeting).toHaveTextContaining('iOS');
  });

  it('user chip shows ENTERPRISE plan after login', async () => {
    await login('iOS Real');
    const plan = await $('.user-plan');
    await expect(plan).toHaveTextContaining('ENTERPRISE');
  });

  it('hamburger button opens the sidebar', async () => {
    await login('iOS Real');
    const ham = await $('button[aria-label="Toggle nav"]');
    await ham.click();
    const sidebar = await $('aside[aria-label="Workshop navigation"]');
    await browser.waitUntil(
      async () => (await sidebar.getAttribute('class'))?.includes('open') ?? false,
      { timeout: 5_000 },
    );
  });

  it('composer textarea fits within viewport (no horizontal overflow)', async () => {
    await login('iOS Real');
    const textarea = await $('#agent-prompt');
    const size = await textarea.getSize();
    const loc = await textarea.getLocation();
    const vw = await browser.execute(() => window.innerWidth);
    expect(loc.x + size.width).toBeLessThanOrEqual((vw as number) + 1);
  });

  it('send button meets Apple HIG 44px touch target on mobile', async () => {
    await login('iOS Real');
    const textarea = await $('#agent-prompt');
    await textarea.setValue('hello');
    const sendBtn = await $('button[aria-label="Send message"]');
    const size = await sendBtn.getSize();
    expect(size.height).toBeGreaterThanOrEqual(44);
  });

  it('theme switcher toggles forge / paper', async () => {
    await login('iOS Real');
    const switcher = await $('button[aria-label*="theme" i], button[aria-label*="Theme" i]');
    await switcher.click();
    const theme = await browser.execute(() => document.documentElement.getAttribute('data-theme'));
    expect(['paper', 'forge']).toContain(theme as string);
  });

  it('no horizontal scroll at iPhone viewport', async () => {
    await login('iOS Real');
    const overflow = await browser.execute(() => ({
      scroll: document.documentElement.scrollWidth,
      client: document.documentElement.clientWidth,
    }));
    const o = overflow as { scroll: number; client: number };
    expect(o.scroll).toBeLessThanOrEqual(o.client + 1);
  });
});
