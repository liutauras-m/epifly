import { prefersReducedMotion } from "../utils/motion-prefs.js";

/**
 * Captures an element's bounding rect for the FLIP technique (First-Last-Invert-Play).
 * Pair with `playFlip(el, recordedRect)` after the layout change to animate the delta.
 */
export function recordRect(el: HTMLElement): DOMRect {
  return el.getBoundingClientRect();
}

/**
 * Plays the inverted transform back to identity, producing a smooth visual
 * transition between two layout states without animating layout properties.
 *
 * Skips when the delta is below 1px / 1% scale (visually imperceptible) and
 * when `prefers-reduced-motion: reduce` is active.
 */
export function playFlip(
  el: HTMLElement,
  from: DOMRect,
  opts: { duration?: number } = {},
): void {
  if (prefersReducedMotion()) return;
  const to = el.getBoundingClientRect();
  const dx = from.left - to.left;
  const dy = from.top - to.top;
  const sx = from.width / (to.width || 1);
  const sy = from.height / (to.height || 1);
  if (
    Math.abs(dx) < 1 &&
    Math.abs(dy) < 1 &&
    Math.abs(sx - 1) < 0.01 &&
    Math.abs(sy - 1) < 0.01
  ) {
    return;
  }
  const dur = opts.duration ?? 320;
  el.style.transformOrigin = "top left";
  el.style.transform = `translate(${dx}px, ${dy}px) scale(${sx}, ${sy})`;
  el.style.transition = "none";
  requestAnimationFrame(() => {
    el.style.transition = `transform ${dur}ms cubic-bezier(0.22, 1, 0.36, 1)`;
    el.style.transform = "";
    el.addEventListener(
      "transitionend",
      () => {
        el.style.transition = "";
        el.style.transformOrigin = "";
      },
      { once: true },
    );
  });
}
