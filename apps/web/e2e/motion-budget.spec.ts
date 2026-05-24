/**
 * Motion budget assertions (Phase 6 — §6 "Per-task duration audit").
 *
 * Walks each of the top-5 task paths from Phase 1.5 and asserts that the
 * sum of all animation-duration + transition-duration values on the active
 * path ≤ 3000 ms (Principle #14).
 *
 * Also checks that no single animation-duration or transition-duration
 * exceeds 400 ms (§6 "Per-transition rule"), except the page-load cascade
 * which is [hierarchy]-tagged and explicitly allowed.
 *
 * Honest framing: a 3000 ms *sum* ≠ 3000 ms *perceived* duration (parallel
 * animations are nearly free). This is a coarse signal catching "death by
 * a thousand small animations". The per-transition 400 ms rule is the sharper
 * signal that catches individual bloat.
 */

import { test, expect } from '@playwright/test';
import { TASK_PATHS } from './fixtures/task-paths.js';

// Cascade allowlist: animation names that are explicitly [hierarchy]-tagged
// and may exceed 400ms as a sum of staggered steps (§6 cascade rule).
const CASCADE_ALLOWLIST = [
  'cascade-in',
  'view-fade-in',  // page-level cross-fade — intentional page-load hierarchy
];

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Parse a CSS time string (e.g. "200ms", "0.2s") to milliseconds. */
function cssTimeToMs(value: string): number {
  const trimmed = value.trim();
  if (trimmed.endsWith('ms')) return parseFloat(trimmed);
  if (trimmed.endsWith('s'))  return parseFloat(trimmed) * 1000;
  return 0;
}

interface AnimInfo {
  element: string;
  property: string;
  durationMs: number;
}

/**
 * Collect all computed animation-duration + transition-duration values from
 * every element currently in the DOM. Returns a flat list for analysis.
 */
async function collectAnimationDurations(page: import('@playwright/test').Page): Promise<AnimInfo[]> {
  return page.evaluate(() => {
    function cssTimeToMsInner(value: string): number {
      const v = value.trim();
      if (v.endsWith('ms')) return parseFloat(v);
      if (v.endsWith('s'))  return parseFloat(v) * 1000;
      return 0;
    }

    const results: Array<{ element: string; property: string; durationMs: number }> = [];

    for (const el of document.querySelectorAll('*')) {
      const style = getComputedStyle(el);
      const tag   = (el as HTMLElement).id
        ? `#${(el as HTMLElement).id}`
        : el.tagName.toLowerCase() + (el.className ? `.${String(el.className).split(' ')[0]}` : '');

      // animation-duration (may be comma-separated for multiple animations)
      const animDur = style.animationDuration ?? '';
      if (animDur && animDur !== '0s' && animDur !== '0ms') {
        for (const part of animDur.split(',')) {
          const ms = cssTimeToMsInner(part);
          if (ms > 0) results.push({ element: tag, property: 'animation-duration', durationMs: ms });
        }
      }

      // transition-duration
      const transDur = style.transitionDuration ?? '';
      if (transDur && transDur !== '0s' && transDur !== '0ms') {
        for (const part of transDur.split(',')) {
          const ms = cssTimeToMsInner(part);
          if (ms > 0) results.push({ element: tag, property: 'transition-duration', durationMs: ms });
        }
      }
    }

    return results;
  });
}

// ── Per-task budget: sum ≤ 3000ms ─────────────────────────────────────────────

test.describe('per-task animation budget ≤ 3000ms', () => {
  const BUDGET_MS = 3000;

  for (const taskPath of TASK_PATHS) {
    test(`${taskPath.label} — total ≤ ${BUDGET_MS}ms`, async ({ page }) => {
      await page.goto(taskPath.startUrl, { waitUntil: 'networkidle' });

      // Give entrance animations a moment to register in computed styles.
      await page.waitForTimeout(100);

      const durations = await collectAnimationDurations(page);
      const total = durations.reduce((sum, d) => sum + d.durationMs, 0);

      const breakdown = durations
        .filter((d) => d.durationMs > 0)
        .sort((a, b) => b.durationMs - a.durationMs)
        .slice(0, 10)
        .map((d) => `  ${d.element} [${d.property}]: ${d.durationMs}ms`)
        .join('\n');

      expect(total, `Task "${taskPath.label}" total animation budget exceeded ${BUDGET_MS}ms (${total}ms).\nTop contributors:\n${breakdown}`).toBeLessThanOrEqual(BUDGET_MS);
    });
  }
});

// ── Per-transition rule: no single animation > 400ms ─────────────────────────

test.describe('no single animation-duration > 400ms (except cascade allowlist)', () => {
  const MAX_SINGLE_MS = 400;

  for (const taskPath of TASK_PATHS) {
    test(`${taskPath.label} — no single animation > ${MAX_SINGLE_MS}ms`, async ({ page }) => {
      await page.goto(taskPath.startUrl, { waitUntil: 'networkidle' });
      await page.waitForTimeout(100);

      const durations = await collectAnimationDurations(page);

      // Get the animation-name for each element so we can check the allowlist.
      const animNames: Record<string, string> = await page.evaluate(() => {
        const out: Record<string, string> = {};
        for (const el of document.querySelectorAll('*')) {
          const style = getComputedStyle(el);
          const name  = style.animationName;
          if (name && name !== 'none') {
            const tag = (el as HTMLElement).id
              ? `#${(el as HTMLElement).id}`
              : el.tagName.toLowerCase();
            out[tag] = name;
          }
        }
        return out;
      });

      const violations = durations.filter((d) => {
        if (d.durationMs <= MAX_SINGLE_MS) return false;
        // Check allowlist (cascade animations are explicitly [hierarchy]-tagged)
        const name = animNames[d.element] ?? '';
        return !CASCADE_ALLOWLIST.some((allowed) => name.includes(allowed));
      });

      const report = violations
        .map((v) => `  ${v.element} [${v.property}]: ${v.durationMs}ms`)
        .join('\n');

      expect(
        violations,
        `Task "${taskPath.label}" has animations exceeding ${MAX_SINGLE_MS}ms (outside cascade allowlist):\n${report}`,
      ).toHaveLength(0);
    });
  }
});
