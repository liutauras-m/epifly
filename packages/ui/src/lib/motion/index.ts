// Phase 2.3 — Motion module (docs/ui-plan.md)
export { springAnimate, type SpringOpts } from "./spring.js";
export { recordRect, playFlip } from "./flip.js";
export { stagger } from "./stagger.js";
export { tap } from "./tap.js";
export { startViewTransition } from "./viewTransition.js";

// Svelte transition helpers — use instead of raw transition strings
export { fadeRise, slideFromRight, cascade, viewFade, toastSlide } from "./transitions.js";

/**
 * tokenSpring — reads spring physics from CSS custom properties and returns
 * SpringOpts so animation values stay inside the token system.
 *
 * Usage: springAnimate(from, to, cb, done, tokenSpring('--spring-snappy'))
 *
 * Only call from browser context (reads computed styles from :root).
 */
export function tokenSpring(
  token: '--spring-snappy' | '--spring-gentle' | '--spring-bouncy',
): import('./spring.js').SpringOpts {
  if (typeof document === 'undefined') return {};
  const val = getComputedStyle(document.documentElement)
    .getPropertyValue(token).trim();
  const [stiffness, damping] = val.split(/\s+/).map(Number);
  if (!stiffness || !damping) return {};
  // Normalise to the 0–1 range springAnimate expects
  return { stiffness: stiffness / 1000, damping: damping / 100 };
}
