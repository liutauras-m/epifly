import { prefersReducedMotion } from "../utils/motion-prefs.js";

export interface SpringOpts {
  stiffness?: number;
  damping?: number;
}

/**
 * Drives a numeric value from `from` to `to` with a critically-damped spring.
 * Calls `onUpdate` each animation frame with the interpolated value.
 *
 * Honours `prefers-reduced-motion: reduce` by snapping to the target value
 * synchronously and invoking `onDone` immediately.
 *
 * Returns a cancel function — invoke it on unmount to stop pending frames.
 */
export function springAnimate(
  from: number,
  to: number,
  onUpdate: (v: number) => void,
  onDone?: () => void,
  opts: SpringOpts = {},
): () => void {
  if (prefersReducedMotion()) {
    onUpdate(to);
    onDone?.();
    return () => {};
  }
  const stiffness = opts.stiffness ?? 0.2;
  const damping = opts.damping ?? 0.8;
  let pos = from;
  let vel = 0;
  let raf = 0;
  function tick() {
    const force = (to - pos) * stiffness;
    vel = (vel + force) * damping;
    pos += vel;
    onUpdate(pos);
    if (Math.abs(to - pos) > 0.01 || Math.abs(vel) > 0.01) {
      raf = requestAnimationFrame(tick);
    } else {
      onUpdate(to);
      onDone?.();
    }
  }
  raf = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(raf);
}
