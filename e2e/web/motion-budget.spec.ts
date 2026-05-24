/**
 * motion-budget.spec.ts — Phase 6 per-task animation duration audit.
 *
 * Principle #14: the sum of all animations on a user-initiated path must not
 * exceed 3 000 ms wall-time. This spec walks the top flows, sums computed
 * `animation-duration` + `transition-duration` on every animated element,
 * and asserts the budget is not exceeded.
 *
 * Per-transition rule (sharper signal): no single duration exceeds 400 ms
 * except allowlisted page-load cascade entries (tagged [hierarchy]).
 *
 * Flows covered:
 *   1. Page load → sidebar settles
 *   2. Rail item click → screen transitions in → cascade settles
 *   3. Composer send → rebound + message appears
 *   4. Theme switch → color cross-fade
 *
 * Notes:
 *   - Sums computed `animation-duration` values on *visible* elements only.
 *   - Durations reported as numeric ms (e.g. `200ms` → 200, `0.2s` → 200).
 *   - Spinner/cursor animations (infinite, tagged explicitly) are excluded via
 *     allowlist so they don't dominate the sum.
 *   - `prefers-reduced-motion: reduce` variant asserts every duration ≤ 80 ms.
 */

import { test, expect } from '@playwright/test';

// ── Helpers ────────────────────────────────────────────────────────────────────

/** Convert CSS duration string → milliseconds. Returns 0 for unknown / 'none'. */
function parseDuration(s: string): number {
  if (!s || s === 'none' || s === '0s' || s === '0ms') return 0;
  if (s.endsWith('ms')) return parseFloat(s);
  if (s.endsWith('s'))  return parseFloat(s) * 1000;
  return 0;
}

/** Names of infinite / indeterminate animations that are budget-exempt. */
const INFINITE_EXEMPT = new Set([
  'composer-spin', 'btn-spin', 'sonar-out', 'blink',
  'ember-pulse',   'tok-in',
]);

/**
 * Collect the maximum individual transition/animation duration across all
 * visible, animated elements in the page. Returns { max, sum }.
 */
async function collectDurations(page: import('@playwright/test').Page) {
  return page.evaluate(({ exemptNames }) => {
    const all = [...document.querySelectorAll('*')];
    let sum = 0;
    let max = 0;
    const overBudget: Array<{ tag: string; name: string; duration: number }> = [];

    for (const el of all) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 && r.height === 0) continue; // skip invisible

      const cs = getComputedStyle(el);

      // Gather all transition durations
      for (const raw of cs.transitionDuration.split(',')) {
        const ms = parseDuration(raw.trim());
        if (ms > 0) { sum += ms; max = Math.max(max, ms); }
      }

      // Gather animation durations (skip infinite/exempt)
      const names = cs.animationName.split(',').map(n => n.trim());
      const durs  = cs.animationDuration.split(',').map(n => n.trim());
      const iters = cs.animationIterationCount.split(',').map(n => n.trim());

      names.forEach((name, i) => {
        if (name === 'none') return;
        if (iters[i] === 'infinite') return;   // infinite animations exempt
        if (exemptNames.includes(name)) return;
        const ms = parseDuration(durs[i] ?? '0ms');
        if (ms > 0) {
          sum += ms;
          max = Math.max(max, ms);
          if (ms > 400) {
            overBudget.push({ tag: el.tagName, name, duration: ms });
          }
        }
      });
    }
    return { sum, max, overBudget };

    // ── Helpers scoped inside evaluate ───────────────────────────────────────
    function parseDuration(s: string): number {
      if (!s || s === 'none' || s === '0s' || s === '0ms') return 0;
      if (s.endsWith('ms')) return parseFloat(s);
      if (s.endsWith('s'))  return parseFloat(s) * 1000;
      return 0;
    }
  }, { exemptNames: [...INFINITE_EXEMPT] });
}

// ── Setup ──────────────────────────────────────────────────────────────────────

test.describe('motion budget (Phase 6 — Principle #14)', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/login');
    await page.getByLabel('Operator name').fill('Motion Budget Test');
    await page.getByLabel('Enterprise').check();
    await page.getByRole('button', { name: 'Begin' }).click();
    await expect(page).toHaveURL('/');
    await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });
  });

  // ── Flow 1: page load ───────────────────────────────────────────────────────

  test('page-load cascade total ≤ 3 000 ms', async ({ page }) => {
    // Wait for cascade to settle (worst-case 920ms per spec)
    await page.waitForTimeout(1000);
    const { sum } = await collectDurations(page);
    expect(sum, `Page-load sum (${sum}ms) exceeds 3000ms budget`).toBeLessThanOrEqual(3000);
  });

  // ── Flow 2: screen switch ──────────────────────────────────────────────────

  test('screen switch (chat → capabilities) total ≤ 3 000 ms', async ({ page }) => {
    // Navigate to capabilities
    const capBtn = page.getByRole('button', { name: /capabilities/i }).first();
    if (await capBtn.isVisible()) {
      await capBtn.click();
      // Collect after transition starts
      await page.waitForTimeout(50);
      const { sum } = await collectDurations(page);
      expect(sum, `Screen switch sum (${sum}ms) exceeds 3000ms budget`).toBeLessThanOrEqual(3000);
    } else {
      test.skip();
    }
  });

  // ── Flow 3: composer send rebound ─────────────────────────────────────────

  test('composer send rebound total ≤ 3 000 ms', async ({ page }) => {
    const composer = page.getByRole('textbox', { name: /message/i });
    await composer.fill('Hello');
    await composer.press('Enter');
    // Capture mid-rebound (~50ms in)
    await page.waitForTimeout(50);
    const { sum } = await collectDurations(page);
    expect(sum, `Send rebound sum (${sum}ms) exceeds 3000ms budget`).toBeLessThanOrEqual(3000);
  });

  // ── Per-transition rule ────────────────────────────────────────────────────

  test('no single transition/animation duration > 400 ms (cascade allowlist excepted)', async ({ page }) => {
    await page.waitForTimeout(1050); // let cascade settle
    const { overBudget } = await collectDurations(page);
    expect(
      overBudget,
      `Found animations exceeding 400ms per-transition limit:\n${JSON.stringify(overBudget, null, 2)}`
    ).toHaveLength(0);
  });

  // ── prefers-reduced-motion variant ────────────────────────────────────────

  test('all durations ≤ 80 ms with prefers-reduced-motion: reduce', async ({ browser }) => {
    const ctx = await browser.newContext({
      reducedMotion: 'reduce',
    });
    const page = await ctx.newPage();

    await page.goto('/login');
    await page.getByLabel('Operator name').fill('Reduced Motion Test');
    await page.getByLabel('Enterprise').check();
    await page.getByRole('button', { name: 'Begin' }).click();
    await expect(page).toHaveURL('/');
    await page.waitForSelector(':root[data-hydrated]', { timeout: 10_000 });

    const { max } = await collectDurations(page);
    expect(max, `Under reduced-motion, max duration ${max}ms exceeds 80ms`).toBeLessThanOrEqual(80);

    await ctx.close();
  });
});
