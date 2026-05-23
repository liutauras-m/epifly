/**
 * Visual regression baseline (Phase 1.3).
 *
 * Captures screenshots for every route × viewport × theme combination and
 * compares against committed baselines in e2e/__screenshots__/.
 *
 * Thresholds (Principle #3):
 *   - Full-page route screenshots: maxDiffPixelRatio 0.005 (0.5%)
 *   - Use `just visual` locally to run inside the official Playwright Docker
 *     image and guarantee byte-identical font rendering across platforms.
 *
 * Dynamic regions are masked via [data-volatile] attributes:
 *   timestamps, "just now" labels, user names, streaming cursors, IDs.
 */

import { test, expect } from '@playwright/test';
import { VISUAL_ROUTES } from '../fixtures/task-paths.js';

const THEMES = ['paper', 'forge'] as const;

const VIEWPORTS = [
  { name: 'iphone-se',  width: 360,  height: 780  },
  { name: 'iphone-16',  width: 390,  height: 844  },
  { name: 'ipad',       width: 768,  height: 1024 },
  { name: 'laptop',     width: 1280, height: 800  },
  { name: 'desktop',    width: 1680, height: 1050 },
] as const;

// Mask any element marked as volatile (timestamps, generated text, cursors).
async function volatileMask(page: import('@playwright/test').Page) {
  return page.locator('[data-volatile]').all();
}

for (const route of VISUAL_ROUTES) {
  for (const theme of THEMES) {
    for (const vp of VIEWPORTS) {
      test(`${route.label} — ${theme} — ${vp.name}`, async ({ page }) => {
        await page.setViewportSize({ width: vp.width, height: vp.height });

        // Set theme before navigation so there's no flash.
        await page.addInitScript((t) => {
          document.documentElement.setAttribute('data-theme', t);
          localStorage.setItem('theme', t);
        }, theme);

        await page.goto(route.path, { waitUntil: 'networkidle' });

        // Wait for any entrance animations to settle (Phase 0 uses CSS transitions).
        await page.waitForTimeout(400);

        const mask = await volatileMask(page);

        await expect(page).toHaveScreenshot(
          `${route.label}-${theme}-${vp.name}.png`,
          {
            maxDiffPixelRatio: 0.005,
            mask,
            fullPage: true,
            animations: 'disabled',
          },
        );
      });
    }
  }
}
