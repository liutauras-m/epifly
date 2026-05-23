/**
 * Svelte 5 transition helpers — Phase 2.3 (docs/ui-plan.md)
 *
 * Wrap Svelte's built-in `transition:` / `in:` / `out:` directives so
 * component authors never write raw `transition: opacity Xms cubic-bezier(…)`
 * strings — all parameters are resolved from CSS tokens.
 *
 * All helpers honour `prefers-reduced-motion: reduce` by clamping to an
 * 80 ms opacity-only fade, matching the CSS keyframe override in keyframes.css.
 *
 * Usage:
 *   <div in:fadeRise>…</div>
 *   <div in:slideFromRight>…</div>
 *   <div in:cascade={{ delay: 40 * index }}>…</div>
 */

import { prefersReducedMotion } from '../utils/motion-prefs.js';

// ── Duration constants (mirrors tokens.css values) ───────────────────────────
const D_FAST    = 120;
const D_NORMAL  = 200;
const D_REDUCED =  80;  // max duration under prefers-reduced-motion

// ── Shared helper ─────────────────────────────────────────────────────────────
function reducedDur(full: number): number {
  return prefersReducedMotion() ? D_REDUCED : full;
}

// ── fadeRise — [continuity] element entering from slightly below ──────────────
export function fadeRise(
  node: Element,
  opts: { duration?: number; delay?: number; y?: number } = {},
) {
  const dur   = reducedDur(opts.duration ?? D_NORMAL);
  const delay = opts.delay ?? 0;
  const y     = prefersReducedMotion() ? 0 : (opts.y ?? 8);

  return {
    delay,
    duration: dur,
    css: (t: number) => {
      const ease = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2; // ease-in-out quad
      return `opacity: ${ease}; transform: translateY(${y * (1 - ease)}px)`;
    },
  };
}

// ── slideFromRight — [feedback] user message / panel entering from right ──────
export function slideFromRight(
  node: Element,
  opts: { duration?: number; delay?: number; x?: number } = {},
) {
  const dur   = reducedDur(opts.duration ?? D_NORMAL);
  const delay = opts.delay ?? 0;
  const x     = prefersReducedMotion() ? 0 : (opts.x ?? 12);

  return {
    delay,
    duration: dur,
    css: (t: number) => {
      const ease = 1 - Math.pow(1 - t, 3); // ease-out cubic
      return `opacity: ${t}; transform: translateX(${x * (1 - ease)}px) scale(${0.96 + 0.04 * ease})`;
    },
  };
}

// ── cascade — [hierarchy] staggered list item enter ───────────────────────────
export function cascade(
  node: Element,
  opts: { duration?: number; delay?: number; y?: number } = {},
) {
  const dur   = reducedDur(opts.duration ?? D_FAST);
  const delay = opts.delay ?? 0;
  const y     = prefersReducedMotion() ? 0 : (opts.y ?? 6);

  return {
    delay,
    duration: dur,
    css: (t: number) => {
      const ease = 1 - Math.pow(1 - t, 2); // ease-out quad
      return `opacity: ${t}; transform: translateY(${y * (1 - ease)}px)`;
    },
  };
}

// ── viewFade — [continuity] route-level cross-fade ────────────────────────────
export function viewFade(
  node: Element,
  opts: { duration?: number } = {},
) {
  const dur = reducedDur(opts.duration ?? D_NORMAL);
  return {
    duration: dur,
    css: (t: number) => `opacity: ${t}`,
  };
}

// ── toastSlide — [feedback] toast entering from bottom ────────────────────────
export function toastSlide(
  node: Element,
  opts: { duration?: number } = {},
) {
  const dur = reducedDur(opts.duration ?? D_NORMAL);
  const y   = prefersReducedMotion() ? 0 : 16;
  return {
    duration: dur,
    css: (t: number) => {
      const ease = 1 - Math.pow(1 - t, 3);
      return `opacity: ${t}; transform: translateY(${y * (1 - ease)}px)`;
    },
  };
}
