/**
 * Reduced-motion visual gate (Phase 7 — §7 + Phase 6 "Reduced-motion gate").
 *
 * With `prefers-reduced-motion: reduce`:
 *  - No `transform` / `translate` animations run (except opacity cross-fades ≤ 80ms)
 *  - Visual baseline must match the static (no-animation) baseline within 0.1%
 *  - Walks all top-5 task paths from Phase 1.5
 *
 * Run locally: `just visual` (inside the Playwright Docker image).
 */

import { test, expect } from '@playwright/test';
import { TASK_PATHS, VISUAL_ROUTES } from '../fixtures/task-paths.js';

// ── Static baseline screenshots ───────────────────────────────────────────────

/**
 * Capture a screenshot of every route with reduced-motion forced and verify it
 * matches the committed baseline within the Principle #3 primitive-gallery tier
 * (0.1%) — reduced-motion pages should be nearly identical to the animated
 * version (same layout, same pixels, just no movement).
 */
test.describe('reduced-motion visual baseline', () => {
  test.use({
    contextOptions: {
      reducedMotion: 'reduce',
    },
  });

  const VIEWPORTS = [
    { name: 'iphone-se', width: 360,  height: 780  },
    { name: 'laptop',    width: 1280, height: 800  },
  ] as const;

  for (const route of VISUAL_ROUTES) {
    for (const vp of VIEWPORTS) {
      test(`${route.label} — reduced-motion — ${vp.name}`, async ({ page }) => {
        await page.setViewportSize({ width: vp.width, height: vp.height });

        // Set a stable theme to avoid cross-test noise.
        await page.addInitScript(() => {
          document.documentElement.setAttribute('data-theme', 'paper');
          localStorage.setItem('theme', 'paper');
        });

        await page.goto(route.path, { waitUntil: 'networkidle' });

        // Give any synchronous entrance setup time to complete (no animations
        // should run but layout shifts from JS-driven positioning still happen).
        await page.waitForTimeout(200);

        const mask = await page.locator('[data-volatile]').all();

        await expect(page).toHaveScreenshot(
          `${route.label}-reduced-motion-${vp.name}.png`,
          {
            maxDiffPixelRatio: 0.001, // 0.1% — primitive-gallery tier (Principle #3)
            mask,
            fullPage: true,
            animations: 'disabled',
          },
        );
      });
    }
  }
});

// ── No transform/translate animations run under reduced-motion ────────────────

test.describe('no transform animations under prefers-reduced-motion: reduce', () => {
  test.use({
    contextOptions: {
      reducedMotion: 'reduce',
    },
  });

  for (const taskPath of TASK_PATHS) {
    test(`${taskPath.label} — zero transform animations`, async ({ page }) => {
      await page.goto(taskPath.startUrl, { waitUntil: 'networkidle' });

      /**
       * Collect all animated elements and verify none have running animations
       * that include transform/translate effects.
       * We check:
       *   1. Web Animations API (getAnimations())
       *   2. CSS computed animation-name (resolves to the keyframe name)
       * Opacity-only animations (≤ 80ms per plan §6) are allowed.
       */
      const violations = await page.evaluate(() => {
        const transformProps = ['transform', 'translate', 'scale', 'rotate', 'skew'];

        function animationUsesTransform(anim: Animation): boolean {
          if (!(anim instanceof CSSAnimation) && !(anim instanceof CSSTransition)) {
            return false;
          }
          // CSSTransition: check the property being transitioned
          if (anim instanceof CSSTransition) {
            return transformProps.some((p) => anim.transitionProperty?.includes(p));
          }
          // CSSAnimation: inspect the keyframe effect
          const effect = anim.effect as KeyframeEffect | null;
          if (!effect) return false;
          const keyframes = effect.getKeyframes();
          return keyframes.some((kf) =>
            transformProps.some((p) => p in kf || Object.keys(kf).some((k) => k.toLowerCase().includes(p))),
          );
        }

        const all = document.querySelectorAll('*');
        const bad: string[] = [];

        for (const el of all) {
          for (const anim of el.getAnimations()) {
            if (anim.playState === 'running' && animationUsesTransform(anim)) {
              const id = (el as HTMLElement).id || (el as HTMLElement).className?.toString().slice(0, 40) || el.tagName;
              bad.push(`${id}: ${(anim as CSSAnimation).animationName ?? (anim as CSSTransition).transitionProperty ?? 'unknown'}`);
            }
          }
        }

        return bad;
      });

      expect(
        violations,
        `Transform animations found under reduced-motion on path "${taskPath.label}": ${violations.join(', ')}`,
      ).toHaveLength(0);
    });
  }
});

// ── Maximum duration clamp: all animations ≤ 80 ms ───────────────────────────

test.describe('animation durations clamped to ≤ 80ms under reduced-motion', () => {
  test.use({
    contextOptions: {
      reducedMotion: 'reduce',
    },
  });

  test('all running animations on home route are ≤ 80ms', async ({ page }) => {
    await page.goto('/', { waitUntil: 'networkidle' });

    const longAnimations = await page.evaluate(() => {
      const MAX_MS = 80;
      const bad: string[] = [];

      for (const el of document.querySelectorAll('*')) {
        for (const anim of el.getAnimations()) {
          const timing = anim.effect?.getTiming?.();
          if (!timing) continue;
          const dur = typeof timing.duration === 'number' ? timing.duration : 0;
          if (dur > MAX_MS) {
            const id = (el as HTMLElement).id || (el as HTMLElement).className?.toString().slice(0, 40) || el.tagName;
            bad.push(`${id}: ${dur}ms`);
          }
        }
      }

      return bad;
    });

    expect(
      longAnimations,
      `Animations longer than 80ms found under reduced-motion: ${longAnimations.join(', ')}`,
    ).toHaveLength(0);
  });
});
